pub mod check;
pub mod keys;
pub mod view;

use crate::app::effect::FlakeEdit;
use crate::app::{App, Effect, Generation};
use crate::flake::edit::Snapshot;
use crate::flake::lock::{self, RevChange};
use crate::flake::{Input, Output, Package, ShellTarget, files, parse};
use ratatui::widgets::TableState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Default)]
pub struct FlakeState {
    pub files: Vec<PathBuf>,
    pub file: usize,
    pub inputs: Vec<Input>,
    pub input_table: TableState,
    pub outputs: Vec<Output>,
    pub output: usize,
    /// Installed packages of the selected output, sorted by name.
    pub packages: Vec<Package>,
    pub package_table: TableState,
    pub marked: HashSet<String>,
    pub updates: HashMap<String, String>,
    pub loading: bool,
    /// Two `nix flake update` runs on one flake.lock can conflict, so only one runs.
    pub updating: bool,
    pub error: Option<String>,
    /// What `z` restores; one level only.
    pub last_edit: Option<Snapshot>,
    generation: Generation,
}

impl FlakeState {
    pub fn selected_file(&self) -> Option<&PathBuf> {
        self.files.get(self.file)
    }

    pub fn selected_output(&self) -> Option<&Output> {
        self.outputs.get(self.output)
    }

    pub fn selected_package(&self) -> Option<&Package> {
        self.packages.get(self.package_table.selected()?)
    }

    pub fn selected_input(&self) -> Option<&Input> {
        self.inputs.get(self.input_table.selected()?)
    }

    /// Like `edit_target`, but also refuses a shell that nui cannot edit in place.
    pub fn editable_target(&self) -> Option<(PathBuf, ShellTarget)> {
        if let Some(reason) = self.selected_output().and_then(|o| o.edit_error.as_ref()) {
            crate::log::output("Error", format!("This shell is read-only: {}", reason));
            return None;
        }
        self.edit_target()
    }

    /// The file and devShell an edit applies to, logging why when there is none.
    pub fn edit_target(&self) -> Option<(PathBuf, ShellTarget)> {
        let file = self.selected_file()?.clone();
        let output = self.selected_output()?;
        match output.shell_target() {
            Some(target) => Some((file, target)),
            None => {
                crate::log::output(
                    "Error",
                    format!(
                        "{} is not a devShell; only devShells can be edited",
                        output.path
                    ),
                );
                None
            }
        }
    }
}

/// Scans for flake files and loads the first one.
pub fn load(app: &mut App) {
    app.flake.files = files::find();
    crate::log::output(
        "Filesystem",
        format!("Found {} nix files", app.flake.files.len()),
    );
    app.flake.file = 0;
    reload(app);
}

/// The devShells an edit can break: its own shell, or every shell for an input edit.
pub fn check_targets(edit: &FlakeEdit, outputs: &[Output]) -> Vec<ShellTarget> {
    match edit {
        FlakeEdit::AddPackage { target, .. }
        | FlakeEdit::RemovePackages { target, .. }
        | FlakeEdit::Pin { target, .. }
        | FlakeEdit::Unpin { target, .. } => vec![target.clone()],
        FlakeEdit::AddInput { .. } | FlakeEdit::RemoveInput(_) => {
            outputs.iter().filter_map(Output::shell_target).collect()
        }
    }
}

pub fn edit(app: &mut App, flake: PathBuf, edit: FlakeEdit) {
    let check = check_targets(&edit, &app.flake.outputs);
    app.effects.push(Effect::Edit { flake, edit, check });
}

pub fn reload(app: &mut App) {
    if app.flake.files.is_empty() {
        app.flake.files = files::find();
    }
    if let Some(file) = app.flake.selected_file() {
        app.effects.push(Effect::LoadFlake(file.clone()));
        app.flake.loading = true;
        app.flake.packages.clear();
    }
}

pub fn inputs_updated(app: &mut App, result: Result<Vec<RevChange>, String>) {
    app.flake.updating = false;
    match result {
        Ok(changes) => {
            crate::log::output("Success", lock::update_summary(&changes));
            reload(app);
        }
        Err(e) => crate::log::output("Error", e),
    }
}

pub fn loaded(app: &mut App, inputs: Vec<Input>, outputs: Vec<Output>) {
    app.flake.inputs = inputs;
    app.flake.outputs = outputs;
    if app.flake.output >= app.flake.outputs.len() {
        app.flake.output = 0;
    }
    fetch_packages(app);
}

pub fn fetch_packages(app: &mut App) {
    let flake = &mut app.flake;
    let Some(output) = flake.selected_output().cloned() else {
        return;
    };
    crate::log::action(
        format!("Fetching package details for {}", output.path),
        format!("Output Type: {}", output.kind.attr()),
    );
    let flake_ref = flake
        .selected_file()
        .map(|f| files::flake_ref(files::dir(f)))
        .unwrap_or_else(|| ".".to_string());

    flake.loading = true;
    flake.error = None;
    flake.packages.clear();
    app.effects.push(Effect::FetchPackages {
        generation: flake.generation.bump(),
        flake_ref,
        output,
    });
}

pub fn packages_loaded(
    app: &mut App,
    generation: Generation,
    result: Result<Vec<Package>, String>,
) {
    let flake = &mut app.flake;
    if generation != flake.generation {
        return;
    }
    flake.loading = false;

    let packages = match result {
        Ok(packages) => packages,
        Err(e) => {
            crate::log::output(
                "Nix Error",
                format!("Error fetching package details: {}", e),
            );
            flake.error = Some(e);
            return;
        }
    };
    crate::log::output(
        "Nix Output",
        format!("Successfully fetched {} package details", packages.len()),
    );
    flake.error = None;

    // Evaluated packages don't say how they were written; recover it from the source.
    let written: HashMap<String, String> = flake
        .selected_output()
        .and_then(|o| o.content.as_deref())
        .map(parse::package_attributes)
        .unwrap_or_default()
        .into_iter()
        .map(|attr| (parse::ref_source(&attr).1.to_string(), attr))
        .collect();

    flake.packages = packages
        .into_iter()
        .map(|mut pkg| {
            pkg.attr = written.get(&pkg.name).cloned().or_else(|| {
                written
                    .iter()
                    .find(|(k, _)| pkg.name.starts_with(*k) || k.starts_with(&pkg.name))
                    .map(|(_, v)| v.clone())
            });
            pkg.source = pkg
                .attr
                .as_deref()
                .and_then(|a| parse::ref_source(a).0)
                .map(str::to_string);
            pkg
        })
        .filter(|pkg| pkg.source.is_some() || !pkg.version.is_empty())
        .collect();
    flake.packages.sort_by(|a, b| a.name.cmp(&b.name));

    flake.updates.clear();
    app.effects.push(Effect::CheckUpdates {
        packages: flake
            .packages
            .iter()
            .map(|p| (p.name.clone(), p.source.clone()))
            .collect(),
        inputs: flake.inputs.clone(),
    });
}
