use crate::action::Action;
use crate::context::Context;
use crate::state::{AppState, Mode};
use std::collections::HashMap;

pub fn handle_edit_action(state: &mut AppState, _context: &Context, action: Action) {
    match action {
        Action::RemoveInput(input_name) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                state
                    .nix_service
                    .remove_input(file.path.clone(), input_name.clone());
            }
        }
        Action::RemoveFlakePackage(pkg_name) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                if let Some(output) = state.domain.outputs.get(state.ui.selected_output_index) {
                    state.nix_service.remove_package(
                        file.path.clone(),
                        output.clone(),
                        pkg_name.clone(),
                    );
                }
            }
        }
        Action::RemoveFlakePackages(pkg_names) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                if let Some(output) = state.domain.outputs.get(state.ui.selected_output_index) {
                    let parts: Vec<&str> = output.path.split('.').collect();
                    let system = parts.first().copied().unwrap_or("x86_64-linux").to_string();
                    let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                    state.nix_service.remove_packages(
                        file.path.clone(),
                        system,
                        shell_name,
                        pkg_names.clone(),
                    );
                    state.ui.selected_packages.clear();
                }
            }
        }
        Action::TogglePackageSelection(pkg_name) => {
            if state.ui.selected_packages.contains(&pkg_name) {
                state.ui.selected_packages.remove(&pkg_name);
            } else {
                state.ui.selected_packages.insert(pkg_name);
            }
        }
        Action::InputPopupSubmit => {
            let name = state.ui.new_input_name.clone();
            let url = state.ui.new_input_url.clone();

            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                let pkg_name = state.ui.selected_package_name.clone();
                let output = state
                    .domain
                    .outputs
                    .get(state.ui.selected_output_index)
                    .cloned();

                state.nix_service.add_input_and_package(
                    file.path.clone(),
                    name,
                    url,
                    pkg_name,
                    output,
                );
            }

            state.ui.is_adding_input = false;
            state.ui.selected_package_name = None;
            state.ui.selected_version = None;
        }
        Action::SetSuggestions(res) => {
            state.domain.suggestions.is_loading = false;
            match res {
                Ok(suggestions) => {
                    state.domain.suggestions.all = suggestions;
                    state.domain.suggestions.error = None;
                    state.update_suggestions();
                }
                Err(e) => {
                    state.domain.suggestions.error = Some(e);
                }
            }
        }
        Action::SetPackageDetails(fetch_id, res) => {
            if fetch_id != state.ui.package_fetch_id {
                return;
            }
            state.ui.fetching_package_details = false;
            state.domain.pending_fetches.clear();
            match res {
                Ok(results) => {
                    crate::log_output(
                        "Nix Output",
                        format!("Successfully fetched {} package details", results.len()),
                    );
                    state.ui.package_fetch_error = None;

                    let mut source_map = HashMap::new();
                    if let Some(output) = state.domain.outputs.get(state.ui.selected_output_index) {
                        if let Some(content) = &output.content {
                            let attrs =
                                crate::nix::query::extract_package_attribute_strings(content);
                            for attr in attrs {
                                if let Some(dot_idx) = attr.find('.') {
                                    let source = &attr[..dot_idx];
                                    let pkg_name = &attr[dot_idx + 1..];
                                    source_map.insert(pkg_name.to_string(), source.to_string());
                                }
                            }
                        }
                    }

                    for (name, (desc, ver, unfree, _)) in results {
                        let source = source_map.get(&name).cloned().unwrap_or_else(|| {
                            source_map
                                .iter()
                                .find(|(k, _)| name.starts_with(*k) || k.starts_with(&name))
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        });

                        if !source.is_empty() || !ver.is_empty() {
                            state
                                .domain
                                .package_info
                                .insert(name, (desc, ver, unfree, source));
                        }
                    }
                    state.check_for_updates();
                }
                Err(e) => {
                    crate::log_output(
                        "Nix Error",
                        format!("Error fetching package details: {}", e),
                    );
                    state.ui.package_fetch_error = Some(e);
                }
            }
        }
        Action::RefreshContext => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                state.ui.fetching_package_details = true;
                state.domain.package_info.clear();
                state.nix_service.refresh_context(file.path.clone());
            }
        }
        Action::AddPackageInfo(name, info) => {
            state.domain.package_info.insert(name.clone(), info.clone());
            if state.mode == Mode::Shell {
                // If the package is in our shell list, update its version if it was a placeholder or inaccurate
                if let Some(pos) = state.shell_packages.iter().position(|p| p == &name) {
                    let version = &info.1;
                    if let Some(at_idx) = name.rfind('@') {
                        let new_name = format!("{}@{}", &name[..at_idx], version);
                        if new_name != name {
                            state.shell_packages[pos] = new_name.clone();
                            state.domain.package_info.remove(&name);
                            state.domain.package_info.insert(new_name, info);
                        }
                    }
                }
            }
        }
        Action::SetContextData(inputs, outputs) => {
            state.domain.inputs = inputs;
            state.domain.outputs = outputs;
            if state.ui.selected_output_index >= state.domain.outputs.len() {
                state.ui.selected_output_index = 0;
            }
            state.fetch_package_details();
        }
        Action::RemovePackage(index) => {
            if state.mode == Mode::Shell && index < state.shell_packages.len() {
                let pkg = state.shell_packages.remove(index);
                state.domain.package_info.remove(&pkg);
                if state.shell_packages.is_empty() {
                    state.ui.shell_package_list_state.select(None);
                } else {
                    let new_index = if index >= state.shell_packages.len() {
                        state.shell_packages.len()
                    } else {
                        index + 1
                    };
                    state.ui.shell_package_list_state.select(Some(new_index));
                }
            }
        }
        Action::RemovePackages(indices) => {
            if state.mode == Mode::Shell {
                let mut sorted_indices = indices.clone();
                sorted_indices.sort_by(|a, b| b.cmp(a));
                for index in sorted_indices {
                    if index < state.shell_packages.len() {
                        let pkg = state.shell_packages.remove(index);
                        state.domain.package_info.remove(&pkg);
                    }
                }
                if state.shell_packages.is_empty() {
                    state.ui.shell_package_list_state.select(None);
                } else {
                    state.ui.shell_package_list_state.select(Some(1));
                }
                state.ui.selected_shell_packages.clear();
            }
        }
        Action::ToggleShellPackageSelection(pkg_name) => {
            if state.ui.selected_shell_packages.contains(&pkg_name) {
                state.ui.selected_shell_packages.remove(&pkg_name);
            } else {
                state.ui.selected_shell_packages.insert(pkg_name);
            }
        }
        Action::UpdateNxvProgress(progress) => {
            state.domain.nxv_update_progress = progress;
        }
        Action::ApplyTemplate(template_name) => {
            if state.mode == Mode::Flake && std::path::Path::new("flake.nix").exists() {
                state.ui.is_confirming_template_overwrite = true;
                state.ui.pending_template_name = Some(template_name);
                return;
            }
            state.apply_template_logic(template_name);
        }
        Action::ConfirmApplyTemplate => {
            if let Some(template_name) = state.ui.pending_template_name.take() {
                state.apply_template_logic(template_name);
            }
            state.ui.is_confirming_template_overwrite = false;
        }
        Action::CancelApplyTemplate => {
            state.ui.pending_template_name = None;
            state.ui.is_confirming_template_overwrite = false;
        }
        Action::TogglePin(pkg_name) => {
            if state.mode == Mode::Shell {
                if state.ui.pinned_packages.contains(&pkg_name) {
                    state.ui.pinned_packages.remove(&pkg_name);
                } else {
                    state.ui.pinned_packages.insert(pkg_name);
                }
            } else if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index)
            {
                if let Some(output) = state.domain.outputs.get(state.ui.selected_output_index) {
                    let mut parts = output.path.split('.');
                    let system = parts.next().unwrap_or("x86_64-linux");
                    let shell_name = parts.next().unwrap_or("default");

                    if pkg_name.starts_with("inputs.") {
                        state.nix_service.unpin_package(
                            file.path.clone(),
                            system.to_string(),
                            shell_name.to_string(),
                            pkg_name.clone(),
                        );
                    } else {
                        state.nix_service.pin_package(
                            file.path.clone(),
                            system.to_string(),
                            shell_name.to_string(),
                            pkg_name.clone(),
                            state.domain.inputs.clone(),
                        );
                    }
                }
            }
        }
        Action::SaveShellTemplate => {
            if let Some(config_dir) = dirs::config_dir() {
                let template_dir = config_dir.join("nui").join("templates");
                let mut filename = state.ui.new_template_filename.clone();
                if !filename.ends_with(".nix") {
                    filename.push_str(".nix");
                }
                let template_path = template_dir.join(&filename);

                let description = &state.ui.new_template_description;
                let packages = &state.shell_packages;

                let mut flake_content = format!(
                    "{{ \n  description = \"{}\";\n  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n  outputs = {{ self, nixpkgs }}: let system = \"x86_64-linux\"; pkgs = nixpkgs.legacyPackages.${{system}}; in {{ \n    devShells.${{system}}.default = pkgs.mkShell {{ \n      packages = [ ",
                    description
                );

                for pkg in packages {
                    let clean_pkg = pkg
                        .split('#')
                        .last()
                        .unwrap_or(pkg)
                        .split('@')
                        .next()
                        .unwrap_or(pkg);
                    flake_content.push_str(&format!("pkgs.{} ", clean_pkg));
                }

                flake_content.push_str("]; \n    }; \n  };\n}}");

                match std::fs::write(&template_path, flake_content) {
                    Ok(_) => {
                        crate::log_output(
                            "Success",
                            format!("Saved shell as template: {}", filename),
                        );
                        state.ui.is_saving_shell_template = false;
                        state.ui.templates = state.nix_service.load_templates();
                    }
                    Err(e) => {
                        crate::log_output("Error", format!("Failed to save template: {}", e));
                    }
                }
            }
        }
        _ => {}
    }
}
