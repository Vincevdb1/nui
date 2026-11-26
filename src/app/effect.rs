use super::action::Action;
use crate::backend::process::{ChildRegistry, cancel_registered};
use crate::backend::search::{self, SearchResult};
use crate::backend::system::{SystemInfo, Tools};
use crate::backend::{github, nix, nxv, templates};
use crate::flake::edit::{self, Changes, EditError, Snapshot};
use crate::flake::locate::Fs;
use crate::flake::{Input, Output, ShellTarget, files, lock, parse};
use crate::pkgref::PackageRef;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// Tags async results so a reply to a superseded request can be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Generation(u64);

impl Generation {
    pub fn bump(&mut self) -> Self {
        self.0 += 1;
        *self
    }
}

pub enum Effect {
    ProbeSystem,
    LoadFlake(PathBuf),
    /// Reads the sources again and runs `nix flake show` only when the inputs changed.
    RefreshFlake {
        path: PathBuf,
        inputs: Vec<Input>,
        outputs: Vec<Output>,
    },
    FetchPackages {
        generation: Generation,
        flake_ref: String,
        output: Output,
    },
    /// `(attr, channel)` lookups that are not in the session cache.
    CheckUpdates(Vec<(String, String)>),
    /// `check` names the devShells to evaluate after the write.
    Edit {
        flake: PathBuf,
        edit: FlakeEdit,
        check: Vec<ShellTarget>,
    },
    Restore(Snapshot),
    /// An empty `inputs` updates all inputs.
    UpdateInputs {
        flake: PathBuf,
        inputs: Vec<String>,
        before: Vec<Input>,
    },
    Search {
        generation: Generation,
        request: search::Request,
    },
    CancelSearch(Generation),
    LockedVersions {
        generation: Generation,
        results: Vec<SearchResult>,
        inputs: Vec<Input>,
    },
    FetchVersions(String),
    ShellMetadata(PackageRef),
    FetchBranches,
    UpdateNxv,
    ApplyTemplate {
        name: String,
        shell: bool,
    },
    SaveTemplate {
        name: String,
        description: String,
        packages: Vec<String>,
    },
}

pub enum FlakeEdit {
    AddPackage {
        target: ShellTarget,
        package: String,
    },
    RemovePackages {
        target: ShellTarget,
        packages: Vec<String>,
    },
    AddInput {
        name: String,
        url: String,
        package: Option<(ShellTarget, String)>,
    },
    RemoveInput(String),
    Pin {
        target: ShellTarget,
        package: String,
        inputs: Vec<Input>,
    },
    Unpin {
        target: ShellTarget,
        package: String,
    },
}

/// The package attribute that refers to `attr` from a flake input.
pub fn input_package(input: &str, attr: &str) -> String {
    format!("{}.legacyPackages.${{system}}.{}", input, attr)
}

/// Concurrent `nix-search` processes for the update check.
const UPDATE_WORKERS: usize = 4;

/// The distinct `(attr, channel)` lookups the update check needs, resolving each package's
/// source input to its upstream channel.
pub fn update_jobs(
    packages: Vec<(String, Option<String>)>,
    inputs: &[Input],
) -> Vec<(String, String)> {
    let mut jobs: Vec<(String, String)> = packages
        .into_iter()
        .map(|(attr, source)| {
            let channel = inputs
                .iter()
                .find(|i| Some(&i.name) == source.as_ref())
                .map(Input::upstream_channel)
                .unwrap_or_else(|| "nixos-unstable".to_string());
            (attr, channel)
        })
        .collect();
    jobs.sort();
    jobs.dedup();
    jobs
}

/// Runs effects on background threads and reports back through the action channel.
pub struct Executor {
    tx: Sender<Action>,
    search_children: ChildRegistry,
    current_search: Arc<AtomicU64>,
}

impl Executor {
    pub fn new(tx: Sender<Action>) -> Self {
        Self {
            tx,
            search_children: ChildRegistry::default(),
            current_search: Arc::default(),
        }
    }

    fn spawn(&self, job: impl FnOnce(&Sender<Action>) + Send + 'static) {
        let tx = self.tx.clone();
        std::thread::spawn(move || job(&tx));
    }

    pub fn run(&self, effect: Effect) {
        match effect {
            Effect::ProbeSystem => self.spawn(|tx| {
                let (system, tools) = std::thread::scope(|scope| {
                    let tools = scope.spawn(Tools::probe);
                    (SystemInfo::probe(), tools.join().unwrap_or_default())
                });
                let _ = tx.send(Action::SystemProbed(system, tools));
            }),
            Effect::LoadFlake(path) => self.spawn(move |tx| {
                let _ = tx.send(load_flake(&path, read_inputs(&path)));
            }),
            Effect::RefreshFlake {
                path,
                inputs,
                outputs,
            } => self.spawn(move |tx| {
                let after = read_inputs(&path);
                if !can_refresh(&inputs, &after, &outputs) {
                    let _ = tx.send(load_flake(&path, after));
                    return;
                }
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                let mut outputs = parse::refresh_outputs(&outputs, &source);
                nix::attach_shell_sources(&Fs, &path, &mut outputs);
                let _ = tx.send(Action::FlakeLoaded(after, outputs));
            }),
            Effect::FetchPackages {
                generation,
                flake_ref,
                output,
            } => self.spawn(move |tx| {
                let result = nix::output_packages(&flake_ref, &output).map_err(|e| e.to_string());
                let _ = tx.send(Action::PackagesLoaded(generation, result));
            }),
            Effect::CheckUpdates(jobs) => {
                let jobs = Arc::new(Mutex::new(jobs));
                for _ in 0..UPDATE_WORKERS {
                    let jobs = Arc::clone(&jobs);
                    self.spawn(move |tx| {
                        loop {
                            let job = jobs.lock().unwrap_or_else(|e| e.into_inner()).pop();
                            let Some((attr, channel)) = job else { break };
                            if let Some(version) = search::latest_version(&attr, &channel) {
                                let _ = tx.send(Action::LatestVersion {
                                    attr,
                                    channel,
                                    version,
                                });
                            }
                        }
                    });
                }
            }
            Effect::Edit { flake, edit, check } => self.spawn(move |tx| {
                if let Some(snapshot) = apply_edit(tx, &flake, edit).filter(|s| !s.is_empty()) {
                    let flake_ref = files::flake_ref(files::dir(&flake));
                    let _ = tx.send(match nix::check_shells(&flake_ref, &check) {
                        Ok(()) => Action::EditApplied(snapshot),
                        Err(nix::CheckError::NotRun(e)) => {
                            crate::log::output(
                                "Warning",
                                format!("Could not check the edit: {}", e),
                            );
                            Action::EditApplied(snapshot)
                        }
                        Err(nix::CheckError::Failed(error)) => {
                            Action::CheckFailed { snapshot, error }
                        }
                    });
                }
                let _ = tx.send(Action::SourcesChanged);
            }),
            Effect::Restore(snapshot) => self.spawn(move |tx| match snapshot.restore() {
                Ok(()) => {
                    let paths: Vec<String> = snapshot
                        .paths()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    crate::log::output("Success", format!("Reverted {}", paths.join(", ")));
                    let _ = tx.send(Action::SourcesChanged);
                }
                Err(e) => crate::log::output("Error", format!("Cannot undo: {}", e)),
            }),
            Effect::UpdateInputs {
                flake,
                inputs,
                before,
            } => self.spawn(move |tx| {
                let result = nix::update_inputs(&files::flake_ref(files::dir(&flake)), &inputs)
                    .map(|()| {
                        let source = std::fs::read_to_string(&flake).unwrap_or_default();
                        let after = parse::inputs(&source, files::read_lock(&flake).as_deref());
                        lock::rev_changes(&before, &after)
                    })
                    .map_err(|e| e.to_string());
                let _ = tx.send(Action::InputsUpdated(result));
            }),
            Effect::Search {
                generation,
                request,
            } => {
                self.cancel_search(generation);
                let reg = Arc::clone(&self.search_children);
                let current = Arc::clone(&self.current_search);
                self.spawn(move |tx| {
                    let is_current = || current.load(Ordering::SeqCst) == generation.0;
                    let result = search::run(&request, &reg, is_current);
                    let _ = tx.send(Action::SearchFinished(generation, result));
                });
            }
            Effect::CancelSearch(generation) => self.cancel_search(generation),
            Effect::LockedVersions {
                generation,
                results,
                inputs,
            } => {
                let reg = Arc::clone(&self.search_children);
                self.spawn(move |tx| {
                    search::locked_versions(&results, &inputs, &reg, |attr, channel, version| {
                        let _ = tx.send(Action::LockedVersion {
                            generation,
                            attr,
                            channel,
                            version,
                        });
                    });
                });
            }
            Effect::FetchVersions(attr) => self.spawn(move |tx| {
                let result = nxv::versions(&attr).map_err(|e| e.to_string());
                let _ = tx.send(Action::VersionsLoaded(attr, result));
            }),
            Effect::ShellMetadata(reference) => self.spawn(move |tx| {
                if let Some(metadata) = nix::metadata(&reference.attr) {
                    let _ = tx.send(Action::ShellMetadata(reference, metadata));
                }
            }),
            Effect::FetchBranches => self.spawn(|tx| {
                let result = github::nixpkgs_branches();
                if let Err(e) = &result {
                    crate::log::output("GitHub Error", e.clone());
                }
                let _ = tx.send(Action::BranchesLoaded(result));
            }),
            Effect::UpdateNxv => self.spawn(|tx| {
                let status = nxv::update(|line| {
                    let _ = tx.send(Action::NxvProgress(Some(line)));
                });
                let _ = tx.send(Action::NxvProgress(None));
                match status {
                    Ok(s) if s.success() => crate::log::info("NXV index updated successfully"),
                    Ok(s) => crate::log::info(format!("NXV update failed with status: {}", s)),
                    Err(e) => crate::log::output("NXV Error", e.to_string()),
                }
            }),
            Effect::ApplyTemplate { name, shell } => {
                self.spawn(move |tx| apply_template(tx, &name, shell))
            }
            Effect::SaveTemplate {
                name,
                description,
                packages,
            } => self.spawn(move |tx| {
                match templates::save_shell(&name, &description, &packages) {
                    Ok(file) => {
                        crate::log::output("Success", format!("Saved shell as template: {}", file))
                    }
                    Err(e) => {
                        crate::log::output("Error", format!("Failed to save template: {}", e))
                    }
                }
                let _ = tx.send(Action::TemplatesListed(templates::list()));
            }),
        }
    }

    /// Kills the in-flight search's processes; their results are dropped by generation.
    fn cancel_search(&self, generation: Generation) {
        self.current_search.store(generation.0, Ordering::SeqCst);
        cancel_registered(&self.search_children);
    }
}

/// Whether new sources can reuse the known outputs: `nix flake show` is needed only when
/// the inputs changed or no outputs are known.
pub fn can_refresh(before: &[Input], after: &[Input], outputs: &[Output]) -> bool {
    before == after && !outputs.is_empty()
}

fn read_inputs(path: &std::path::Path) -> Vec<Input> {
    let source = std::fs::read_to_string(path).unwrap_or_default();
    parse::inputs(&source, files::read_lock(path).as_deref())
}

fn load_flake(path: &std::path::Path, inputs: Vec<Input>) -> Action {
    crate::log::action(
        "Loading flake outputs",
        format!("nix flake show {} --json --impure", path.display()),
    );
    let outputs = nix::flake_show(path).unwrap_or_else(|e| {
        crate::log::output("Nix Error", format!("Failed to fetch outputs: {}", e));
        Vec::new()
    });
    Action::FlakeLoaded(inputs, outputs)
}

/// Applies an edit and logs the outcome; returns what it replaced when it wrote.
fn apply_edit(tx: &Sender<Action>, flake: &std::path::Path, edit: FlakeEdit) -> Option<Snapshot> {
    let report = |result: Result<Changes, EditError>, success: String, failure: &str| match result
        .and_then(|changes| edit::apply(&changes))
    {
        Ok(snapshot) => {
            crate::log::output("Success", success);
            Some(snapshot)
        }
        Err(e) => {
            crate::log::output("Error", format!("{}: {}", failure, e));
            None
        }
    };
    let fs = &Fs;

    match edit {
        FlakeEdit::AddPackage { target, package } => add_package(tx, flake, &target, &package),
        FlakeEdit::RemovePackages { target, packages } => report(
            edit::remove_packages(fs, flake, &target, &packages),
            format!(
                "Removed {} from {}.{}",
                packages.join(", "),
                target.system,
                target.name
            ),
            "Failed to remove packages",
        ),
        FlakeEdit::AddInput { name, url, package } => {
            let mut snapshot = report(
                edit::edit_file(fs, flake, |src| edit::add_input(src, &name, &url)),
                format!("Added input: {}", name),
                "Failed to write flake.nix",
            )?;
            // One snapshot for both writes, so one undo reverts both.
            if let Some((target, attr)) = package
                && let Some(added) = add_package(tx, flake, &target, &input_package(&name, &attr))
            {
                snapshot.merge(added);
            }
            Some(snapshot)
        }
        FlakeEdit::RemoveInput(name) => report(
            edit::edit_file(fs, flake, |src| edit::remove_input(src, &name)),
            format!("Removed input: {}", name),
            "Failed to remove input",
        ),
        FlakeEdit::Pin {
            target,
            package,
            inputs,
        } => report(
            edit::pin_package(fs, flake, &target, &package, &inputs),
            format!("Pinned package {}", package),
            &format!("Failed to pin {}", package),
        ),
        FlakeEdit::Unpin { target, package } => report(
            edit::unpin_package(fs, flake, &target, &package),
            format!("Unpinned package {}", package),
            &format!("Failed to unpin {}", package),
        ),
    }
}

/// Adds in place, or to a new managed file when nui cannot find the shell.
fn add_package(
    tx: &Sender<Action>,
    flake: &std::path::Path,
    target: &ShellTarget,
    package: &str,
) -> Option<Snapshot> {
    let result = match edit::add_package(&Fs, flake, target, package) {
        Err(EditError::ShellNotFound(_) | EditError::AmbiguousShell(..)) => {
            edit::create_managed(&Fs, flake, &target.name, package).and_then(|(changes, line)| {
                let snapshot = edit::apply(&changes)?;
                let file = edit::managed_path(flake, &target.name);
                let _ = tx.send(Action::Notice(format!(
                    "nui could not find devShell {}.{} in the flake source, so it wrote {} to {}.\n\nAdd this line to the shell:\n\n{}",
                    target.system,
                    target.name,
                    package,
                    file.display(),
                    line
                )));
                Ok(snapshot)
            })
        }
        result => result.and_then(|changes| edit::apply(&changes)),
    };
    match result {
        Ok(snapshot) => {
            crate::log::output(
                "Success",
                format!("Added {} to {}.{}", package, target.system, target.name),
            );
            Some(snapshot)
        }
        Err(e) => {
            crate::log::output("Error", format!("Failed to add package: {}", e));
            None
        }
    }
}

fn apply_template(tx: &Sender<Action>, name: &str, shell: bool) {
    let Some(path) = templates::path(name).filter(|p| p.exists()) else {
        crate::log::output("Error", format!("Template not found: {}", name));
        return;
    };
    crate::log::output("Debug", format!("Applying template: {}", name));
    if shell {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let _ = tx.send(Action::ShellTemplateLoaded(templates::packages(&content)));
            }
            Err(e) => crate::log::output("Error", format!("Failed to read template: {}", e)),
        }
    } else {
        match std::fs::copy(&path, "flake.nix") {
            Ok(_) => {
                crate::log::output("Success", "Applied template to flake.nix");
                let _ = tx.send(Action::FlakeChanged);
            }
            Err(e) => crate::log::output("Error", format!("Failed to apply template: {}", e)),
        }
    }
}
