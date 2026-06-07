use crate::action::Action;
use crate::context::Context;
use crate::state::{AppState, Mode, domain};

pub fn handle_search_action(state: &mut AppState, _context: &Context, action: Action) {
    match action {
        Action::PackageSearchSubmitVersions => {
            if let Some(i) = state.ui.package_search_state.selected() {
                if let Some(result) = state.domain.package_search_results.get(i) {
                    let package_name = result.name.clone();
                    state.ui.selected_package_description = Some(result.description.clone());
                    state.ui.selected_package_is_unfree = result.is_unfree;
                    state.update(Action::FetchVersions(package_name));
                }
            }
        }
        Action::PackageSearchSubmitDirect => {
            if let Some(i) = state.ui.package_search_state.selected() {
                if let Some(result) = state.domain.package_search_results.get(i) {
                    let package_name = result.name.clone();
                    if state.mode == Mode::Shell {
                        let version = result
                            .versions
                            .iter()
                            .find(|v| v.channel == "system")
                            .map(|v| v.version.clone())
                            .unwrap_or_else(|| {
                                if let Some(sys_v) = state.domain.system_nixpkgs_version.as_ref() {
                                    result
                                        .versions
                                        .iter()
                                        .find(|v| v.channel == *sys_v || sys_v.starts_with(&v.channel))
                                        .map(|v| v.version.clone())
                                        .unwrap_or_else(|| {
                                            result
                                                .versions
                                                .first()
                                                .map(|v| v.version.clone())
                                                .unwrap_or_else(|| "Unknown".to_string())
                                        })
                                } else {
                                    result
                                        .versions
                                        .first()
                                        .map(|v| v.version.clone())
                                        .unwrap_or_else(|| "Unknown".to_string())
                                }
                            });
                        let pkg_to_add =
                            if let Some(hash) = state.domain.system_nixpkgs_hash.as_ref() {
                                format!("system/{}#{}@{}", hash, package_name, version)
                            } else if let Some(hash) = result.hash.as_ref() {
                                format!("nixpkgs/{}#{}@{}", hash, package_name, version)
                            } else {
                                format!("{}@{}", package_name, version)
                            };

                        if !state.shell_packages.contains(&pkg_to_add) {
                            state.shell_packages.push(pkg_to_add.clone());
                            state.domain.package_info.insert(
                                pkg_to_add.clone(),
                                (
                                    result.description.clone(),
                                    version,
                                    result.is_unfree,
                                    String::new(),
                                ),
                            );
                            state.fetch_shell_package_metadata(pkg_to_add);
                        }
                        state.ui.is_adding_package = false;
                        state.ui.package_search_query = String::new();
                        state.domain.package_search_results = Vec::new();
                    } else {
                        if let Some(file) =
                            state.domain.nix_files.get(state.ui.selected_nix_file_index)
                        {
                            let flake_path = file.path.clone();
                            if let Some(output) =
                                state.domain.outputs.get(state.ui.selected_output_index)
                            {
                                let tx = state.tx.clone();
                                let package_name = package_name.clone();
                                let output = output.clone();
                                std::thread::spawn(move || {
                                    use crate::nix::traits::NixEditor;
                                    if let Err(e) = crate::nix::modifier::FlakeEditor::add_package(
                                        &flake_path,
                                        &output,
                                        &package_name,
                                    ) {
                                        crate::log_output(
                                            "Error",
                                            format!("Failed to add package: {}", e),
                                        );
                                    } else {
                                        crate::log_output(
                                            "Success",
                                            format!("Added {} to {}", package_name, output.path),
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
                                                    let _ = tx.send(Action::SetLockedVersion(
                                                        result.name.clone(),
                                                        channel,
                                                        version,
                                                    ));
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
            state
                .domain
                .package_updates
                .insert(attribute.clone(), version.clone());
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
        Action::FetchShellPackageVersions(index, pkg) => {
            state.ui.is_adding_package = true;
            state.ui.editing_shell_package_index = Some(index);
            state.ui.is_selecting_version = true;
            state.ui.is_fetching_versions = true;
            state.ui.version_fetch_error = None;
            state.ui.selected_package_name = Some(pkg.clone());

            if let Some(pkg_id) = state.shell_packages.get(index) {
                if let Some((desc, _, unfree, _)) = state.domain.package_info.get(pkg_id) {
                    state.ui.selected_package_description = Some(desc.clone());
                    state.ui.selected_package_is_unfree = *unfree;
                }
            }

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
                                if let Some(res) = state
                                    .domain
                                    .package_search_results
                                    .iter()
                                    .find(|r| &r.name == pkg_name)
                                {
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
                        format!(
                            "system/{}#{}@{}",
                            version_info.hash, pkg_name, version_info.version
                        )
                    } else {
                        format!(
                            "nixpkgs/{}#{}@{}",
                            version_info.hash, pkg_name, version_info.version
                        )
                    };

                    if let Some(index) = state.ui.editing_shell_package_index.take() {
                        if index < state.shell_packages.len() {
                            let old_pkg_id = state.shell_packages[index].clone();
                            state.shell_packages[index] = pinned_pkg.clone();
                            state.domain.package_info.remove(&old_pkg_id);
                            state.domain.package_info.insert(
                                pinned_pkg,
                                (
                                    state
                                        .ui
                                        .selected_package_description
                                        .clone()
                                        .unwrap_or_default(),
                                    version_info.version.clone(),
                                    state.ui.selected_package_is_unfree,
                                    String::new(),
                                ),
                            );
                        }
                    } else if !state.shell_packages.contains(&pinned_pkg) {
                        state.shell_packages.push(pinned_pkg.clone());
                        state.domain.package_info.insert(
                            pinned_pkg,
                            (
                                state
                                    .ui
                                    .selected_package_description
                                    .clone()
                                    .unwrap_or_default(),
                                version_info.version.clone(),
                                state.ui.selected_package_is_unfree,
                                String::new(),
                            ),
                        );
                    }
                }
                state.ui.is_adding_package = false;
                state.ui.is_selecting_version = false;
                state.ui.selected_package_name = None;
                state.ui.selected_package_description = None;
                state.ui.package_search_query = String::new();
                state.domain.package_search_results = Vec::new();
                state.ui.editing_shell_package_index = None;
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
                            let prefixed_pkg =
                                format!("{}.legacyPackages.${{system}}.{}", input_name, pkg_name);
                            use crate::nix::traits::NixEditor;
                            if let Err(e) = crate::nix::modifier::FlakeEditor::add_package(
                                &flake_path,
                                output,
                                &prefixed_pkg,
                            ) {
                                crate::log_output("Error", format!("Failed to add package: {}", e));
                            } else {
                                crate::log_output(
                                    "Nix",
                                    format!("Added package {} to output {}", pkg_name, output.path),
                                );
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
        _ => {}
    }
}
