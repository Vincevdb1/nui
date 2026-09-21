use crate::action::Action;
use crate::context::Context;
use crate::state::{AppState, Mode};

pub mod edit;
pub mod search;

pub fn handle_nix_action(state: &mut AppState, context: &Context, action: Action) {
    match action {
        Action::SwitchMode => {
            if state.mode == Mode::Flake {
                state.mode = Mode::Shell;
                state.ui.selected_index = 1;
                state.domain.nix_files = Vec::new();
                state.domain.inputs = Vec::new();
                state.domain.outputs = Vec::new();
                if state.shell_packages.is_empty() {
                    state.open_package_search();
                }
            } else {
                state.mode = Mode::Flake;
                state.ui.selected_index = 1;
                if !state.domain.package_info.is_empty() {
                    state.ui.package_table_state.select(Some(1));
                }
                let nix_files = crate::context::find_nix_files();
                state.domain.nix_files = nix_files;
                if let Some(file) = state.domain.nix_files.first() {
                    let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
                    let lock_path = file
                        .path
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join("flake.lock");
                    let lock_content = std::fs::read_to_string(lock_path).ok();
                    state.domain.inputs =
                        crate::nix::parser::extract_inputs(&flake_content, lock_content.as_deref());
                    state.domain.outputs = crate::nix::parser::fetch_outputs(
                        file.path.parent().unwrap_or(std::path::Path::new(".")),
                    )
                    .unwrap_or_default();
                    state.fetch_package_details();
                }
            }
        }
        Action::Log(entry) => {
            state.domain.logs.push(entry);
            let total_lines = crate::components::command_log::count_lines(&state.domain.logs);
            if total_lines > 0 {
                state.ui.command_log_state.select(Some(total_lines - 1));
            }
        }
        Action::StartShell(packages) => {
            state.mode = Mode::Shell;
            state.shell_packages = packages;
            state.ui.selected_index = 1;
            state.should_quit = true;
        }
        Action::Quit => state.should_quit = true,

        // Delegate to sub-modules
        a if is_search_action(&a) => search::handle_search_action(state, context, a),
        a => edit::handle_edit_action(state, context, a),
    }
}

fn is_search_action(action: &Action) -> bool {
    matches!(
        action,
        Action::PackageSearchSubmitVersions
            | Action::PackageSearchSubmitDirect
            | Action::SetPackageSearchResults(..)
            | Action::UpdatePackageVersion(_, _)
            | Action::SetLockedVersion(..)
            | Action::FetchVersions(_)
            | Action::FetchShellPackageVersions(_, _)
            | Action::SetVersions(_)
            | Action::SelectVersion(_)
            | Action::SelectInputForPackage(_)
    )
}
