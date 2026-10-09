use super::action::Action;
use crate::backend::process::{ChildRegistry, cancel_registered};
use crate::backend::search::{self, SearchResult};
use crate::backend::system::{SystemInfo, Tools};
use crate::backend::{github, nix, nxv, templates};
use crate::flake::edit::{self, EditError};
use crate::flake::{Input, Output, ShellTarget, files, parse};
use crate::pkgref::PackageRef;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

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
    FetchPackages {
        generation: Generation,
        flake_ref: String,
        output: Output,
    },
    CheckUpdates {
        packages: Vec<(String, Option<String>)>,
        inputs: Vec<Input>,
    },
    Edit {
        flake: PathBuf,
        edit: FlakeEdit,
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
                let _ = tx.send(Action::SystemProbed(SystemInfo::probe(), Tools::probe()));
            }),
            Effect::LoadFlake(path) => self.spawn(move |tx| {
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                let inputs = parse::inputs(&source, files::read_lock(&path).as_deref());
                let outputs = nix::flake_show(&path).unwrap_or_else(|e| {
                    crate::log::output("Nix Error", format!("Failed to fetch outputs: {}", e));
                    Vec::new()
                });
                let _ = tx.send(Action::FlakeLoaded(inputs, outputs));
            }),
            Effect::FetchPackages {
                generation,
                flake_ref,
                output,
            } => self.spawn(move |tx| {
                let result = nix::output_packages(&flake_ref, &output).map_err(|e| e.to_string());
                let _ = tx.send(Action::PackagesLoaded(generation, result));
            }),
            Effect::CheckUpdates { packages, inputs } => {
                for (attr, source) in packages {
                    let channel = inputs
                        .iter()
                        .find(|i| Some(&i.name) == source.as_ref())
                        .map(Input::upstream_channel)
                        .unwrap_or_else(|| "nixos-unstable".to_string());
                    self.spawn(move |tx| {
                        if let Some(version) = search::latest_version(&attr, &channel) {
                            let _ = tx.send(Action::LatestVersion { attr, version });
                        }
                    });
                }
            }
            Effect::Edit { flake, edit } => self.spawn(move |tx| {
                if apply_edit(&flake, edit) {
                    let _ = tx.send(Action::FlakeChanged);
                }
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

/// Applies an edit and logs the outcome; returns whether the flake should be reloaded.
fn apply_edit(flake: &std::path::Path, edit: FlakeEdit) -> bool {
    let report = |result: Result<(), EditError>, success: String, failure: &str| match result {
        Ok(()) => {
            crate::log::output("Success", success);
            true
        }
        Err(e) => {
            crate::log::output("Error", format!("{}: {}", failure, e));
            false
        }
    };

    match edit {
        FlakeEdit::AddPackage { target, package } => {
            let result = edit::rewrite(flake, |src| edit::add_package(src, &target, &package));
            report(
                result,
                format!("Added {} to {}.{}", package, target.system, target.name),
                "Failed to add package",
            );
            true
        }
        FlakeEdit::RemovePackages { target, packages } => {
            let result = edit::rewrite(flake, |src| {
                Ok(edit::remove_packages(src, &target, &packages))
            });
            report(
                result,
                format!(
                    "Removed {} from {}.{}",
                    packages.join(", "),
                    target.system,
                    target.name
                ),
                "Failed to remove packages",
            );
            true
        }
        FlakeEdit::AddInput { name, url, package } => {
            let result = edit::rewrite(flake, |src| Ok(edit::add_input(src, &name, &url)));
            if !report(
                result,
                format!("Added input: {}", name),
                "Failed to write flake.nix",
            ) {
                return true;
            }
            if let Some((target, attr)) = package {
                let package = input_package(&name, &attr);
                let result = edit::rewrite(flake, |src| edit::add_package(src, &target, &package));
                report(
                    result,
                    format!("Added {} to {}.{}", package, target.system, target.name),
                    "Failed to add package",
                );
            }
            true
        }
        FlakeEdit::RemoveInput(name) => {
            let result = edit::rewrite(flake, |src| Ok(edit::remove_input(src, &name)));
            report(
                result,
                format!("Removed input: {}", name),
                "Failed to remove input",
            );
            true
        }
        FlakeEdit::Pin {
            target,
            package,
            inputs,
        } => {
            let result = edit::rewrite(flake, |src| {
                edit::pin_package(src, &target, &package, &inputs)
            });
            report(
                result,
                format!("Pinned package {}", package),
                &format!("Failed to pin {}", package),
            )
        }
        FlakeEdit::Unpin { target, package } => {
            let result = edit::rewrite(flake, |src| edit::unpin_package(src, &target, &package));
            report(
                result,
                format!("Unpinned package {}", package),
                &format!("Failed to unpin {}", package),
            )
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
