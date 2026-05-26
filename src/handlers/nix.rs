use crate::state::{AppState, Mode, domain};
use crate::action::Action;
use crate::context::Context;
use std::collections::HashMap;

pub fn handle_nix_action(state: &mut AppState, _context: &Context, action: Action) {
    match action {
        Action::SwitchMode => {
            if state.mode == Mode::Flake {
                state.mode = Mode::Shell;
                state.ui.selected_index = 1;
                state.domain.nix_files = Vec::new();
                state.domain.inputs = Vec::new();
                state.domain.outputs = Vec::new();
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
                    let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                    let lock_content = std::fs::read_to_string(lock_path).ok();
                    state.domain.inputs = crate::nix::flake::extract_inputs(&flake_content, lock_content.as_deref());
                    state.domain.outputs = crate::nix::flake::fetch_outputs(
                        file.path.parent().unwrap_or(std::path::Path::new(".")),
                    )
                    .unwrap_or_default();
                    state.fetch_package_details();
                }
            }
        }
        Action::RemoveInput(input_name) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                let flake_path = file.path.clone();
                let tx = state.tx.clone();
                let input_name = input_name.clone();
                std::thread::spawn(move || {
                    let content = std::fs::read_to_string(&flake_path).unwrap_or_default();
                    match crate::nix::flake::remove_input(&content, &input_name) {
                        Ok(new_content) => {
                            if let Err(e) = std::fs::write(&flake_path, new_content) {
                                crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                            } else {
                                crate::log_output("Success", format!("Removed input: {}", input_name));
                            }
                        }
                        Err(e) => {
                            crate::log_output("Error", format!("Failed to remove input: {}", e));
                        }
                    }
                    let _ = tx.send(Action::RefreshContext);
                });
            }
        }
        Action::RemoveFlakePackage(pkg_name) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                let flake_path = file.path.clone();
                if let Some(output) = state
                    .domain
                    .outputs
                    .get(state.ui.selected_output_index)
                {
                    let parts: Vec<&str> = output.path.split('.').collect();
                    let system = parts.get(0).copied().unwrap_or("x86_64-linux").to_string();
                    let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                    let tx = state.tx.clone();
                    let pkg_name = pkg_name.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = crate::nix::flake::remove_package(
                            &flake_path,
                            &system,
                            &shell_name,
                            &pkg_name,
                        ) {
                            crate::log_output(
                                "Error",
                                format!("Failed to remove package: {}", e),
                            );
                        } else {
                            crate::log_output(
                                "Success",
                                format!("Removed {} from {}", pkg_name, shell_name),
                            );
                        }
                        let _ = tx.send(Action::RefreshContext);
                    });
                }
            }
        }
        Action::RemoveFlakePackages(pkg_names) => {
            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                let flake_path = file.path.clone();
                if let Some(output) = state
                    .domain
                    .outputs
                    .get(state.ui.selected_output_index)
                {
                    let parts: Vec<&str> = output.path.split('.').collect();
                    let system = parts.get(0).copied().unwrap_or("x86_64-linux").to_string();
                    let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                    let tx = state.tx.clone();
                    let pkg_names = pkg_names.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = crate::nix::flake::remove_packages(
                            &flake_path,
                            &system,
                            &shell_name,
                            &pkg_names,
                        ) {
                            crate::log_output(
                                "Error",
                                format!("Failed to remove packages: {}", e),
                            );
                        } else {
                            crate::log_output(
                                "Success",
                                format!("Removed {} packages from {}", pkg_names.len(), shell_name),
                            );
                        }
                        let _ = tx.send(Action::RefreshContext);
                    });
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
        Action::PackageSearchSubmitVersions => {
            if let Some(i) = state.ui.package_search_state.selected() {
                if let Some(result) = state.domain.package_search_results.get(i) {
                    let package_name = result.name.clone();
                    state.update(Action::FetchVersions(package_name));
                }
            }
        }
        Action::PackageSearchSubmitDirect => {
            if let Some(i) = state.ui.package_search_state.selected() {
                if let Some(result) = state.domain.package_search_results.get(i) {
                    let package_name = result.name.clone();
                    if state.mode == Mode::Shell {
                        let version = result.versions.first().map(|v| v.version.clone()).unwrap_or_else(|| "Unknown".to_string());
                        let pkg_to_add = if let Some(hash) = state.domain.system_nixpkgs_hash.as_ref() {
                            format!("system/{}#{}@{}", hash, package_name, version)
                        } else if let Some(hash) = result.hash.as_ref() {
                            format!("nixpkgs/{}#{}@{}", hash, package_name, version)
                        } else {
                            format!("{}@{}", package_name, version)
                        };

                        if !state.shell_packages.contains(&pkg_to_add) {
                            state.shell_packages.push(pkg_to_add);
                        }
                        state.ui.is_adding_package = false;
                    } else {
                        if let Some(file) =
                            state.domain.nix_files.get(state.ui.selected_nix_file_index)
                        {
                            let flake_path = file.path.clone();
                            if let Some(output) = state
                                .domain
                                .outputs
                                .get(state.ui.selected_output_index)
                            {
                                let parts: Vec<&str> = output.path.split('.').collect();
                                let system = parts.get(0).copied().unwrap_or("x86_64-linux").to_string();
                                let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                                let tx = state.tx.clone();
                                std::thread::spawn(move || {
                                    if let Err(e) = crate::nix::flake::add_package(
                                        &flake_path,
                                        &system,
                                        &shell_name,
                                        &package_name,
                                    ) {
                                        crate::log_output(
                                            "Error",
                                            format!("Failed to add package: {}", e),
                                        );
                                    } else {
                                        crate::log_output(
                                            "Success",
                                            format!("Added {} to {}", package_name, shell_name),
                                        );
                                    }
                                    let _ = tx.send(Action::RefreshContext);
                                });
                            }
                        }
                        state.ui.is_adding_package = false;
                    }
                }
            }
        }
        Action::InputPopupSubmit => {
            let name = state.ui.new_input_name.clone();
            let url = state.ui.new_input_url.clone();

            if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                let flake_path = file.path.clone();
                let pkg_name = state.ui.selected_package_name.clone();
                let selected_output_index = state.ui.selected_output_index;
                let outputs = state.domain.outputs.clone();
                let tx = state.tx.clone();

                std::thread::spawn(move || {
                    let content = std::fs::read_to_string(&flake_path).unwrap_or_default();
                    let new_content = crate::nix::flake::add_input(&content, &name, &url);

                    if let Some(pkg_name) = pkg_name {
                        if let Some(output) = outputs.get(selected_output_index) {
                            let parts: Vec<&str> = output.path.split('.').collect();
                            let (system, shell_name) = if parts.len() >= 2 {
                                (parts[0], parts[1])
                            } else {
                                ("x86_64-linux", parts[0])
                            };

                            let prefixed_pkg = format!("{}.legacyPackages.{}.{}", name, system, pkg_name);

                            if let Err(e) = std::fs::write(&flake_path, &new_content) {
                                crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                                let _ = tx.send(Action::RefreshContext);
                                return;
                            }

                            if let Err(e) = crate::nix::flake::add_package(
                                &flake_path,
                                system,
                                shell_name,
                                &prefixed_pkg,
                            ) {
                                crate::log_output("Error", format!("Failed to add package: {}", e));
                            } else {
                                crate::log_output(
                                    "Success",
                                    format!("Added {} to {} ({})", prefixed_pkg, shell_name, system),
                                );
                            }
                        }
                    } else {
                        if let Err(e) = std::fs::write(&flake_path, new_content) {
                            crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                        } else {
                            crate::log_output("Success", format!("Added input: {}", name));
                        }
                    }
                    let _ = tx.send(Action::RefreshContext);
                });
            }

            state.ui.is_adding_input = false;
            state.ui.selected_package_name = None;
            state.ui.selected_version = None;
        }
        Action::Log(entry) => {
            state.domain.logs.push(entry);
            let total_lines = crate::components::command_log::count_lines(&state.domain.logs);
            if total_lines > 0 {
                state.ui.command_log_state.select(Some(total_lines - 1));
            }
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
        Action::SetPackageSearchResults(res) => {
            state.ui.is_searching_packages = false;
            match res {
                Ok(results) => {
                    state.ui.package_search_error = None;
                    state.domain.package_search_results = results;
                    if !state.domain.package_search_results.is_empty() {
                        state.ui.package_search_state.select(Some(0));

                        if state.mode == Mode::Flake {
                            let top_results = state
                                .domain
                                .package_search_results
                                .iter()
                                .take(10)
                                .cloned()
                                .collect::<Vec<_>>();
                            let inputs = state.domain.inputs.clone();
                            let tx = state.tx.clone();

                            std::thread::spawn(move || {
                                for result in top_results {
                                    if result.source_input.is_some() {
                                        continue;
                                    }
                                    for input in &inputs {
                                        if input.url.contains("nixpkgs") {
                                            if let Some(rev) = &input.rev {
                                                if let Some(version) =
                                                    domain::fetch_accurate_version(
                                                        rev.clone(),
                                                        result.name.clone(),
                                                    )
                                                {
                                                    let channel = domain::extract_channel(input);
                                                    let _ = tx.send(
                                                        Action::SetLockedVersion(
                                                            result.name.clone(),
                                                            channel,
                                                            version,
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    } else {
                        state.ui.package_search_state.select(None);
                    }
                }
                Err(e) => {
                    state.ui.package_search_error = Some(e.clone());
                    crate::log_output("Nix Error", format!("Error searching packages: {}", e));
                }
            }
        }
        Action::UpdatePackageVersion(attribute, version) => {
            state.domain.package_updates.insert(attribute.clone(), version.clone());
            for result in &mut state.domain.package_search_results {
                if result.name == attribute {
                    for cv in &mut result.versions {
                        if cv.channel.contains("nixpkgs")
                            || cv.channel.contains("nixos")
                            || cv.channel.len() == 40
                        {
                            cv.version = version.clone();
                        }
                    }
                }
            }
        }
        Action::SetLockedVersion(attribute, channel, version) => {
            for result in &mut state.domain.package_search_results {
                if result.name == attribute {
                    for cv in &mut result.versions {
                        if cv.channel == channel {
                            cv.locked_version = Some(version.clone());
                        }
                    }
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
                    if let Some(output) = state
                        .domain
                        .outputs
                        .get(state.ui.selected_output_index)
                    {
                        if let Some(content) = &output.content {
                            let attrs =
                                crate::nix::flake::extract_package_attribute_strings(content);
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
                            source_map.iter()
                                .find(|(k, _)| name.starts_with(*k) || k.starts_with(&name))
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        });
                        
                        if !source.is_empty() || !ver.is_empty() {
                            state.domain
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
                
                let flake_path = file.path.clone();
                let tx = state.tx.clone();

                std::thread::spawn(move || {
                    let content = std::fs::read_to_string(&flake_path).unwrap_or_default();
                    let lock_path = flake_path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                    let lock_content = std::fs::read_to_string(lock_path).ok();
                    let inputs = crate::nix::flake::extract_inputs(&content, lock_content.as_deref());
                    
                    let outputs = match crate::nix::flake::fetch_outputs(
                        flake_path.parent().unwrap_or(std::path::Path::new(".")),
                    ) {
                        Ok(outputs) => outputs,
                        Err(e) => {
                            crate::log_output("Nix Error", format!("Failed to fetch outputs: {}", e));
                            Vec::new()
                        }
                    };
                    
                    let _ = tx.send(Action::SetContextData(inputs, outputs));
                });
            }
        }
        Action::AddPackageInfo(name, info) => {
            state.domain.package_info.insert(name, info);
        }
        Action::SetContextData(inputs, outputs) => {
            state.domain.inputs = inputs;
            state.domain.outputs = outputs;
            if state.ui.selected_output_index >= state.domain.outputs.len() {
                state.ui.selected_output_index = 0;
            }
            state.fetch_package_details();
        }
        Action::StartShell(packages) => {
            state.mode = Mode::Shell;
            state.shell_packages = packages;
            state.ui.selected_index = 1;
            state.should_quit = true;
        }
        Action::RemovePackage(index) => {
            if state.mode == Mode::Shell && index < state.shell_packages.len() {
                state.shell_packages.remove(index);
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
                        state.shell_packages.remove(index);
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
        Action::FetchVersions(pkg) => {
            state.ui.is_selecting_version = true;
            state.ui.is_fetching_versions = true;
            state.ui.version_fetch_error = None;
            state.ui.selected_package_name = Some(pkg.clone());
            state.domain.package_versions.clear();
            state.ui.version_list_state.select(None);

            let tx = state.tx.clone();
            std::thread::spawn(move || {
                let res = domain::fetch_package_versions(&pkg);
                let _ = tx.send(Action::SetVersions(res));
            });
        }
        Action::SetVersions(res) => {
            state.ui.is_fetching_versions = false;
            match res {
                Ok(mut versions) => {
                    if state.mode == Mode::Shell {
                        if let Some(sys_hash) = &state.domain.system_nixpkgs_hash {
                            let mut pkg_version = "Unknown".to_string();
                            let mut is_unfree = false;
                            if let Some(pkg_name) = &state.ui.selected_package_name {
                                if let Some(res) = state.domain.package_search_results.iter().find(|r| &r.name == pkg_name) {
                                    is_unfree = res.is_unfree;
                                    if let Some(v) = res.versions.first() {
                                        pkg_version = v.version.clone();
                                    }
                                }
                            }

                            if !versions.iter().any(|v| v.hash == *sys_hash) {
                                versions.push(domain::VersionInfo {
                                    version: pkg_version,
                                    hash: sys_hash.clone(),
                                    date: "".to_string(),
                                    is_unfree,
                                    is_system: true,
                                });
                            } else {
                                if let Some(v) = versions.iter_mut().find(|v| v.hash == *sys_hash) {
                                    v.is_system = true;
                                }
                            }
                        }
                    }

                    state.domain.package_versions = versions;
                    if !state.domain.package_versions.is_empty() {
                        state.ui.version_list_state.select(Some(0));
                    }
                }
                Err(e) => {
                    state.ui.version_fetch_error = Some(e.clone());
                    crate::log_output("Nix Error", format!("Error fetching versions: {}", e));
                }
            }
        }
        Action::SelectVersion(version_info) => {
            if state.mode == Mode::Shell {
                if let Some(pkg_name) = state.ui.selected_package_name.clone() {
                    let pinned_pkg = if version_info.is_system {
                        format!("system/{}#{}@{}", version_info.hash, pkg_name, version_info.version)
                    } else {
                        format!("nixpkgs/{}#{}@{}", version_info.hash, pkg_name, version_info.version)
                    };
                    state.shell_packages.push(pinned_pkg);
                }
                state.ui.is_adding_package = false;
                state.ui.is_selecting_version = false;
                state.ui.selected_package_name = None;
                return;
            }

            if let Some(_pkg_name) = state.ui.selected_package_name.clone() {
                state.ui.selected_version = Some(version_info.clone());
                state.ui.is_adding_input = true;
                state.ui.is_adding_package = false;
                state.ui.is_selecting_version = false;
                state.ui.new_input_name = format!("nixpkgs-{}", &version_info.hash[..7]);
                state.ui.new_input_url = format!("github:NixOS/nixpkgs/{}", version_info.hash);
                state.ui.input_cursor = 1;
            }
        }
        Action::SelectInputForPackage(input_name) => {
            if state.mode == Mode::Shell {
                if let Some(pkg_name) = state.ui.selected_package_name.clone() {
                    let pkg = format!("{}#{}", input_name, pkg_name);
                    state.shell_packages.push(pkg);
                }
                state.ui.is_adding_package = false;
                state.ui.is_selecting_version = false;
                state.ui.selected_package_name = None;
                return;
            }

            if let Some(pkg_name) = state.ui.selected_package_name.clone() {
                if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                    let flake_path = file.path.clone();
                    let selected_output_index = state.ui.selected_output_index;
                    let outputs = state.domain.outputs.clone();
                    let tx = state.tx.clone();

                    std::thread::spawn(move || {
                        if let Some(output) = outputs.get(selected_output_index) {
                            let parts: Vec<&str> = output.path.split('.').collect();
                            let (system, shell_name) = if parts.len() >= 2 {
                                (parts[0], parts[1])
                            } else {
                                ("x86_64-linux", parts[0])
                            };

                            let prefixed_pkg = format!("{}.legacyPackages.{}.{}", input_name, system, pkg_name);
                            if let Err(e) = crate::nix::flake::add_package(&flake_path, system, shell_name, &prefixed_pkg) {
                                crate::log_output("Error", format!("Failed to add package: {}", e));
                            } else {
                                crate::log_output("Nix", format!("Added package {} to output {}", pkg_name, output.path));
                            }
                            let _ = tx.send(Action::RefreshContext);
                        }
                    });
                }
            }
            state.ui.is_adding_package = false;
            state.ui.is_selecting_version = false;
            state.ui.selected_package_name = None;
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
            } else if let Some(file) = state.domain.nix_files.get(state.ui.selected_nix_file_index) {
                if let Some(output) = state.domain.outputs.get(state.ui.selected_output_index) {
                    let mut parts = output.path.split('.');
                    let system = parts.next().unwrap_or("x86_64-linux");
                    let shell_name = parts.next().unwrap_or("default");

                    if pkg_name.starts_with("inputs.") {
                        // Unpin
                        match crate::nix::flake::unpin_package(
                            &file.path,
                            system,
                            shell_name,
                            &pkg_name,
                        ) {
                            Ok(_) => {
                                crate::log_output("Success", format!("Unpinned package {}", pkg_name));
                                let _ = state.tx.send(Action::RefreshContext);
                            }
                            Err(e) => {
                                crate::log_output(
                                    "Error",
                                    format!("Failed to unpin {}: {}", pkg_name, e),
                                );
                            }
                        }
                    } else {
                        // Pin
                        match crate::nix::flake::pin_package(
                            &file.path,
                            system,
                            shell_name,
                            &pkg_name,
                            &state.domain.inputs,
                        ) {
                            Ok(_) => {
                                crate::log_output("Success", format!("Pinned package {}", pkg_name));
                                let _ = state.tx.send(Action::RefreshContext);
                            }
                            Err(e) => {
                                crate::log_output(
                                    "Error",
                                    format!("Failed to pin {}: {}", pkg_name, e),
                                );
                            }
                        }
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
                    let clean_pkg = pkg.split('#').last().unwrap_or(pkg).split('@').next().unwrap_or(pkg);
                    flake_content.push_str(&format!("pkgs.{} ", clean_pkg));
                }
                
                flake_content.push_str("]; \n    }; \n  };\n}}");
                
                match std::fs::write(&template_path, flake_content) {
                    Ok(_) => {
                        crate::log_output("Success", format!("Saved shell as template: {}", filename));
                        state.ui.is_saving_shell_template = false;
                        state.ui.templates = crate::state::load_templates();
                    }
                    Err(e) => {
                        crate::log_output("Error", format!("Failed to save template: {}", e));
                    }
                }
            }
        }
        Action::Quit => state.should_quit = true,
        _ => {}
    }
}
