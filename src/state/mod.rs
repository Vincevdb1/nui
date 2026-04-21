pub mod domain;
pub mod ui;

pub use domain::{ChannelVersion, DomainData, SearchResult};
pub use ui::UiState;

use crate::action::Action;
use crate::context::find_nix_files;
use crate::nix::flake::{add_nixpkgs_input, add_package, extract_configurations, extract_inputs};
use crate::nix::suggestions::Suggestions;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Flake,
    Shell,
}

pub struct AppState {
    pub mode: Mode,
    pub shell_packages: Vec<String>,
    pub ui: UiState,
    pub domain: DomainData,
    pub should_quit: bool,

    // Unified channel for background tasks
    pub tx: Sender<Action>,
    pub rx: Receiver<Action>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Mode::Flake, Vec::new())
    }
}

impl AppState {
    pub fn new(mode: Mode, shell_packages: Vec<String>) -> Self {
        let (tx, rx) = mpsc::channel();

        crate::components::command_log::init_logger(tx.clone());
        crate::command_log("Initializing NUI Application...");

        let (nix_files, inputs, configurations) = if mode == Mode::Flake {
            let nix_files = find_nix_files();
            crate::log_output("Filesystem", format!("Found {} nix files", nix_files.len()));

            let (inputs, configurations) = if let Some(file) = nix_files.first() {
                let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
                let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                let lock_content = std::fs::read_to_string(lock_path).ok();
                (
                    extract_inputs(&flake_content, lock_content.as_deref()),
                    extract_configurations(&flake_content),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            (nix_files, inputs, configurations)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        let mut app = Self {
            mode,
            shell_packages,
            ui: UiState::default(),
            domain: DomainData {
                nix_files,
                inputs,
                configurations,
                nh_version: domain::get_tool_version("nh"),
                nxv_version: domain::get_tool_version("nxv"),
                ..Default::default()
            },
            should_quit: false,
            tx,
            rx,
        };

        if app.mode == Mode::Shell {
            app.ui.selected_index = 1;
        }

        if app.mode == Mode::Flake {
            app.fetch_package_details();
        }
        app
    }

    pub fn update(&mut self, action: Action) {
        match action {
            Action::Tick => {
                self.ui.throbber_state.calc_next();
                self.process_background_results();

                // Search throttle logic
                if self.ui.is_adding_package
                    && self.ui.last_search_time.elapsed() > std::time::Duration::from_millis(300)
                    && self.ui.package_search_query != self.ui.last_search_query
                {
                    self.ui.last_search_query = self.ui.package_search_query.clone();
                    if self.ui.package_search_query.is_empty() {
                        self.domain.package_search_results.clear();
                        self.ui.package_search_state.select(None);
                    } else if !self.ui.is_searching_packages {
                        self.perform_package_search();
                    }
                }
            }
            Action::Quit => self.should_quit = true,
            Action::NextTab => {
                self.ui.selected_index = if self.mode == Mode::Shell {
                    1
                } else {
                    match self.ui.selected_index {
                        1 => 2,
                        2 => 3,
                        3 => 4,
                        4 => 1,
                        _ => 1,
                    }
                };
            }
            Action::PreviousTab => {
                self.ui.selected_index = if self.mode == Mode::Shell {
                    1
                } else {
                    match self.ui.selected_index {
                        1 => 4,
                        2 => 1,
                        3 => 2,
                        4 => 3,
                        _ => 1,
                    }
                };
            }
            Action::SelectTab(index) => {
                self.ui.selected_index = index;
            }
            Action::SwitchMode => {
                if self.mode == Mode::Flake {
                    self.mode = Mode::Shell;
                    self.ui.selected_index = 1;
                    // Clear domain data that is specific to Flake mode
                    self.domain.nix_files = Vec::new();
                    self.domain.inputs = Vec::new();
                    self.domain.configurations = Vec::new();
                } else {
                    self.mode = Mode::Flake;
                    self.ui.selected_index = 1;
                    // Re-initialize Flake mode data
                    let nix_files = find_nix_files();
                    self.domain.nix_files = nix_files;
                    if let Some(file) = self.domain.nix_files.first() {
                        let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
                        let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                        let lock_content = std::fs::read_to_string(lock_path).ok();
                        self.domain.inputs = extract_inputs(&flake_content, lock_content.as_deref());
                        self.domain.configurations = extract_configurations(&flake_content);
                        self.fetch_package_details();
                    }
                }
            }
            Action::MoveDown => match self.ui.selected_index {
                1 => {
                    if self.mode == Mode::Shell && !self.shell_packages.is_empty() {
                        let i = match self.ui.shell_package_list_state.selected() {
                            Some(i) => {
                                if i >= self.shell_packages.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        self.ui.shell_package_list_state.select(Some(i));
                    }
                }
                2 => {
                    if !self.domain.nix_files.is_empty() {
                        self.ui.selected_nix_file_index =
                            (self.ui.selected_nix_file_index + 1) % self.domain.nix_files.len();
                        self.update(Action::RefreshContext);
                    }
                }
                4 => {
                    if !self.domain.configurations.is_empty() {
                        self.ui.selected_configuration_index =
                            (self.ui.selected_configuration_index + 1)
                                % self.domain.configurations.len();
                        self.fetch_package_details();
                    }
                }
                5 => {
                    if !self.domain.logs.is_empty() {
                        let total_lines =
                            crate::components::command_log::count_lines(&self.domain.logs);
                        let i = match self.ui.command_log_state.selected() {
                            Some(i) => {
                                if i >= total_lines - 1 {
                                    i
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        self.ui.command_log_state.select(Some(i));
                    }
                }
                _ => {}
            },
            Action::MoveUp => match self.ui.selected_index {
                1 => {
                    if self.mode == Mode::Shell && !self.shell_packages.is_empty() {
                        let i = match self.ui.shell_package_list_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    self.shell_packages.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        self.ui.shell_package_list_state.select(Some(i));
                    }
                }
                2 => {
                    if !self.domain.nix_files.is_empty() {
                        self.ui.selected_nix_file_index = if self.ui.selected_nix_file_index == 0 {
                            self.domain.nix_files.len() - 1
                        } else {
                            self.ui.selected_nix_file_index - 1
                        };
                        self.update(Action::RefreshContext);
                    }
                }
                4 => {
                    if !self.domain.configurations.is_empty() {
                        self.ui.selected_configuration_index =
                            if self.ui.selected_configuration_index == 0 {
                                self.domain.configurations.len() - 1
                            } else {
                                self.ui.selected_configuration_index - 1
                            };
                        self.fetch_package_details();
                    }
                }
                5 => {
                    if !self.domain.logs.is_empty() {
                        let i = match self.ui.command_log_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    0
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        self.ui.command_log_state.select(Some(i));
                    }
                }
                _ => {}
            },
            Action::MovePackageSelectionDown => {
                let count = self.domain.package_info.len();
                if count > 0 {
                    let i = match self.ui.package_table_state.selected() {
                        Some(i) => {
                            if i >= count {
                                1
                            } else {
                                i + 1
                            }
                        }
                        None => 1,
                    };
                    self.ui.package_table_state.select(Some(i));
                }
            }
            Action::MovePackageSelectionUp => {
                let count = self.domain.package_info.len();
                if count > 0 {
                    let i = match self.ui.package_table_state.selected() {
                        Some(i) => {
                            if i <= 1 {
                                count
                            } else {
                                i - 1
                            }
                        }
                        None => 1,
                    };
                    self.ui.package_table_state.select(Some(i));
                }
            }
            Action::RemoveFlakePackage(pkg_name) => {
                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let flake_path = file.path.clone();
                    if let Some(config) = self
                        .domain
                        .configurations
                        .get(self.ui.selected_configuration_index)
                    {
                        let parts: Vec<&str> = config.path.split('.').collect();
                        let system = parts.get(0).copied().unwrap_or("x86_64-linux").to_string();
                        let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                        let tx = self.tx.clone();
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
            Action::OpenAddPackage => {
                self.ui.is_adding_package = true;
                self.ui.is_selecting_version = false;
                self.ui.package_search_query.clear();
                self.domain.package_search_results.clear();
                self.domain.package_versions.clear();
            }
            Action::OpenAddInput => {
                self.ui.is_adding_input = true;
                self.ui.new_input_name.clear();
                self.ui.new_input_url.clear();
                self.ui.input_cursor = 0;
                self.start_fetching_suggestions();
            }
            Action::ClosePopup => {
                self.ui.is_adding_package = false;
                self.ui.is_adding_input = false;
                self.ui.is_selecting_version = false;
            }
            Action::PackageSearchChar(c) => {
                self.ui.package_search_query.push(c);
                self.ui.last_search_time = std::time::Instant::now();
            }
            Action::PackageSearchBackspace => {
                self.ui.package_search_query.pop();
                self.ui.last_search_time = std::time::Instant::now();
            }
            Action::PackageSearchSubmitVersions => {
                if let Some(i) = self.ui.package_search_state.selected() {
                    if let Some(result) = self.domain.package_search_results.get(i) {
                        let package_name = result.name.clone();
                        self.update(Action::FetchVersions(package_name));
                    }
                }
            }
            Action::PackageSearchSubmitDirect => {
                if let Some(i) = self.ui.package_search_state.selected() {
                    if let Some(result) = self.domain.package_search_results.get(i) {
                        let package_name = result.name.clone();
                        if self.mode == Mode::Shell {
                            let pkg_to_add = if let Some(hash) = &result.hash {
                                format!("nixpkgs/{}#{}", hash, package_name)
                            } else {
                                package_name
                            };

                            if !self.shell_packages.contains(&pkg_to_add) {
                                self.shell_packages.push(pkg_to_add);
                                if self.ui.shell_package_list_state.selected().is_none() {
                                    self.ui.shell_package_list_state.select(Some(0));
                                }
                            }
                            self.ui.is_adding_package = false;
                        } else {
                            if let Some(file) =
                                self.domain.nix_files.get(self.ui.selected_nix_file_index)
                            {
                                let flake_path = file.path.clone();
                                if let Some(config) = self
                                    .domain
                                    .configurations
                                    .get(self.ui.selected_configuration_index)
                                {
                                    let parts: Vec<&str> = config.path.split('.').collect();
                                    let system = parts.get(0).copied().unwrap_or("x86_64-linux").to_string();
                                    let shell_name = parts.get(1).copied().unwrap_or("default").to_string();

                                    let tx = self.tx.clone();
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
                            self.ui.is_adding_package = false;
                        }
                    }
                }
            }
            Action::TogglePackageDetails => {
                self.ui.is_showing_package_details = !self.ui.is_showing_package_details;
            }
            Action::MoveSearchSelectionDown => {
                let i = match self.ui.package_search_state.selected() {
                    Some(i) => {
                        if i >= self.domain.package_search_results.len().saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.ui.package_search_state.select(Some(i));
            }
            Action::MoveSearchSelectionUp => {
                let i = match self.ui.package_search_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.domain.package_search_results.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.ui.package_search_state.select(Some(i));
            }
            Action::BackToPackageSearch => {
                self.ui.is_selecting_version = false;
            }
            Action::MoveVersionSelectionDown => {
                if !self.domain.package_versions.is_empty() {
                    let i = match self.ui.version_list_state.selected() {
                        Some(i) => {
                            if i >= self.domain.package_versions.len().saturating_sub(1) {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    self.ui.version_list_state.select(Some(i));
                }
            }
            Action::MoveVersionSelectionUp => {
                if !self.domain.package_versions.is_empty() {
                    let i = match self.ui.version_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                self.domain.package_versions.len().saturating_sub(1)
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    self.ui.version_list_state.select(Some(i));
                }
            }
            Action::InputPopupChar(c) => {
                match self.ui.input_cursor {
                    0 | 1 => self.ui.new_input_name.push(c),
                    2 => self.ui.new_input_url.push(c),
                    _ => {}
                }
                self.update_suggestions();
            }
            Action::InputPopupBackspace => {
                match self.ui.input_cursor {
                    0 | 1 => {
                        self.ui.new_input_name.pop();
                    }
                    2 => {
                        self.ui.new_input_url.pop();
                    }
                    _ => {}
                }
                self.update_suggestions();
            }
            Action::InputPopupSubmit => {
                // Add input logic
            }
            Action::NextInputField => {
                self.ui.input_cursor = (self.ui.input_cursor + 1) % 3;
            }
            Action::PreviousInputField => {
                self.ui.input_cursor = if self.ui.input_cursor == 0 {
                    2
                } else {
                    self.ui.input_cursor - 1
                };
            }
            Action::MoveSuggestionDown => {
                if !self.domain.suggestions.filtered.is_empty() {
                    self.domain.suggestions.selected_index =
                        (self.domain.suggestions.selected_index + 1)
                            % self.domain.suggestions.filtered.len();
                    self.domain
                        .suggestions
                        .list_state
                        .select(Some(self.domain.suggestions.selected_index));
                }
            }
            Action::MoveSuggestionUp => {
                if !self.domain.suggestions.filtered.is_empty() {
                    self.domain.suggestions.selected_index =
                        if self.domain.suggestions.selected_index == 0 {
                            self.domain.suggestions.filtered.len() - 1
                        } else {
                            self.domain.suggestions.selected_index - 1
                        };
                    self.domain
                        .suggestions
                        .list_state
                        .select(Some(self.domain.suggestions.selected_index));
                }
            }
            Action::Log(entry) => {
                self.domain.logs.push(entry);
                let total_lines = crate::components::command_log::count_lines(&self.domain.logs);
                if total_lines > 0 {
                    self.ui.command_log_state.select(Some(total_lines - 1));
                }
            }
            Action::SetSuggestions(suggestions) => {
                self.domain.suggestions.all = suggestions;
                self.update_suggestions();
                self.domain.suggestions.is_loading = false;
            }
            Action::SetPackageSearchResults(res) => {
                self.ui.is_searching_packages = false;
                match res {
                    Ok(results) => {
                        self.domain.package_search_results = results;
                        if !self.domain.package_search_results.is_empty() {
                            self.ui.package_search_state.select(Some(0));

                            // Refine versions for Flake mode
                            if self.mode == Mode::Flake {
                                let top_results = self
                                    .domain
                                    .package_search_results
                                    .iter()
                                    .take(10)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                let inputs = self.domain.inputs.clone();
                                let tx = self.tx.clone();

                                std::thread::spawn(move || {
                                    for result in top_results {
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
                            self.ui.package_search_state.select(None);
                        }
                    }
                    Err(e) => {
                        crate::log_output("Nix Error", format!("Error searching packages: {}", e));
                    }
                }
            }
            Action::UpdatePackageVersion(attribute, version) => {
                self.domain.package_updates.insert(attribute.clone(), version.clone());
                for result in &mut self.domain.package_search_results {
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
                for result in &mut self.domain.package_search_results {
                    if result.name == attribute {
                        for cv in &mut result.versions {
                            if cv.channel == channel {
                                cv.locked_version = Some(version.clone());
                            }
                        }
                    }
                }
            }
            Action::SetPackageDetails(res) => {
                self.ui.fetching_package_details = false;
                self.domain.pending_fetches.clear();
                match res {
                    Ok(results) => {
                        crate::log_output(
                            "Nix Output",
                            format!("Successfully fetched {} package details", results.len()),
                        );
                        self.ui.package_fetch_error = None;

                        let mut source_map = HashMap::new();
                        if let Some(config) = self
                            .domain
                            .configurations
                            .get(self.ui.selected_configuration_index)
                        {
                            if let Some(content) = &config.content {
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
                            let source = source_map.get(&name).cloned().unwrap_or_default();
                            self.domain
                                .package_info
                                .insert(name, (desc, ver, unfree, source));
                        }
                        self.check_for_updates();
                    }
                    Err(e) => {
                        crate::log_output(
                            "Nix Error",
                            format!("Error fetching package details: {}", e),
                        );
                        self.ui.package_fetch_error = Some(e);
                    }
                }
            }
            Action::RefreshContext => {
                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let content = std::fs::read_to_string(&file.path).unwrap_or_default();
                    let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                    let lock_content = std::fs::read_to_string(lock_path).ok();
                    self.domain.inputs = extract_inputs(&content, lock_content.as_deref());
                    self.domain.configurations = extract_configurations(&content);
                    if self.ui.selected_configuration_index >= self.domain.configurations.len() {
                        self.ui.selected_configuration_index = 0;
                    }
                    self.fetch_package_details();
                }
            }
            Action::FetchPackageDetails => {
                self.fetch_package_details();
            }
            Action::StartShell(packages) => {
                self.mode = Mode::Shell;
                self.shell_packages = packages;
                self.ui.selected_index = 1;
                self.should_quit = true;
            }
            Action::UpdateShellPackages(packages) => {
                self.shell_packages = packages;
            }
            Action::RemovePackage(index) => {
                if self.mode == Mode::Shell && index < self.shell_packages.len() {
                    self.shell_packages.remove(index);
                    if self.shell_packages.is_empty() {
                        self.ui.shell_package_list_state.select(None);
                    } else {
                        let new_index = if index >= self.shell_packages.len() {
                            self.shell_packages.len() - 1
                        } else {
                            index
                        };
                        self.ui.shell_package_list_state.select(Some(new_index));
                    }
                }
            }
            Action::FetchVersions(pkg) => {
                self.ui.is_selecting_version = true;
                self.ui.is_fetching_versions = true;
                self.ui.version_fetch_error = None;
                self.ui.selected_package_name = Some(pkg.clone());
                self.domain.package_versions.clear();
                self.ui.version_list_state.select(None);

                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let res = domain::fetch_package_versions(&pkg);
                    let _ = tx.send(Action::SetVersions(res));
                });
            }
            Action::SetVersions(res) => {
                self.ui.is_fetching_versions = false;
                match res {
                    Ok(versions) => {
                        self.domain.package_versions = versions;
                        if !self.domain.package_versions.is_empty() {
                            self.ui.version_list_state.select(Some(0));
                        }
                    }
                    Err(e) => {
                        self.ui.version_fetch_error = Some(e.clone());
                        crate::log_output("Nix Error", format!("Error fetching versions: {}", e));
                    }
                }
            }
            Action::SelectVersion(version_info) => {
                if self.mode == Mode::Shell {
                    if let Some(pkg_name) = self.ui.selected_package_name.clone() {
                        let pinned_pkg = format!("nixpkgs/{}#{}", version_info.hash, pkg_name);
                        self.shell_packages.push(pinned_pkg);
                    }
                    self.ui.is_adding_package = false;
                    self.ui.is_selecting_version = false;
                    self.ui.selected_package_name = None;
                    return;
                }

                if let Some(pkg_name) = self.ui.selected_package_name.clone() {
                    if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                        let flake_path = file.path.clone();
                        let selected_config_index = self.ui.selected_configuration_index;
                        let configurations = self.domain.configurations.clone();
                        let tx = self.tx.clone();

                        std::thread::spawn(move || {
                            let content = std::fs::read_to_string(&flake_path).unwrap_or_default();

                            // 1. Add nixpkgs input
                            let new_content = add_nixpkgs_input(&content, &version_info.hash);
                            if let Err(e) = std::fs::write(&flake_path, new_content) {
                                crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                            } else {
                                // 2. Add package
                                if let Some(config) = configurations.get(selected_config_index) {
                                    if config.config_type == "devShells" {
                                        let parts: Vec<&str> = config.path.split('.').collect();
                                        let (system, shell_name) = if parts.len() >= 2 {
                                            (parts[0], parts[1])
                                        } else {
                                            ("x86_64-linux", parts[0])
                                        };

                                        let prefixed_pkg = format!(
                                            "nixpkgs-{}.legacyPackages.{}.{}",
                                            version_info.hash, system, pkg_name
                                        );

                                        if let Err(e) =
                                            add_package(&flake_path, system, shell_name, &prefixed_pkg)
                                        {
                                            crate::log_output(
                                                "Error",
                                                format!("Failed to add package: {}", e),
                                            );
                                        } else {
                                            crate::log_output(
                                                "Success",
                                                format!(
                                                    "Added {} to {} ({})",
                                                    prefixed_pkg, shell_name, system
                                                ),
                                            );
                                        }
                                    } else {
                                        crate::log_output(
                                            "Warning",
                                            format!(
                                                "Version pinning is currently only supported for devShells, not {}",
                                                config.config_type
                                            ),
                                        );
                                    }
                                }
                            }
                            let _ = tx.send(Action::RefreshContext);
                        });
                    }
                }
                self.ui.is_adding_package = false;
                self.ui.is_selecting_version = false;
                self.ui.selected_package_name = None;
            }
        }
    }

    fn process_background_results(&mut self) {
        while let Ok(action) = self.rx.try_recv() {
            self.update(action);
        }
    }

    fn fetch_package_details(&mut self) {
        if let Some(config) = self
            .domain
            .configurations
            .get(self.ui.selected_configuration_index)
        {
            let config_type = config.config_type.clone();
            let config_name = config.path.clone();

            crate::log_action(
                format!("Fetching package details for {}", config_name),
                format!("Config Type: {}", config_type),
            );

            let flake_path =
                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let parent = file.path.parent().unwrap_or(std::path::Path::new("."));
                    let p = parent.to_string_lossy().to_string();
                    if p.is_empty() || p == "." {
                        ".".to_string()
                    } else {
                        format!("./{}", p)
                    }
                } else {
                    ".".to_string()
                };

            self.ui.fetching_package_details = true;
            self.ui.package_fetch_error = None;
            self.domain.package_info.clear();
            self.domain.pending_fetches.clear();

            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let res =
                    crate::nix::Package::fetch_from_config(&flake_path, &config_type, &config_name);
                let _ = tx.send(Action::SetPackageDetails(res));
            });
        }
    }

    fn perform_package_search(&mut self) {
        if self.ui.package_search_query.is_empty() {
            self.domain.package_search_results.clear();
            return;
        }

        self.ui.is_searching_packages = true;
        let query = self.ui.package_search_query.clone();
        let tx = self.tx.clone();

        let mut search_targets: Vec<String> = if self.mode == Mode::Shell {
            vec!["nxv".to_string()]
        } else {
            let mut targets = Vec::new();
            for input in &self.domain.inputs {
                if input.url.contains("nixpkgs") {
                    targets.push(domain::extract_upstream_channel(input));
                }
            }
            if targets.is_empty() {
                targets.push("nixos-unstable".to_string());
            }
            targets
        };

        search_targets.sort();
        search_targets.dedup();
        self.domain.searched_channels = search_targets.clone();

        let mode = self.mode.clone();
        std::thread::spawn(move || {
            let mut results_map: HashMap<String, SearchResult> = HashMap::new();

            if mode == Mode::Shell {
                if let Ok(packages) = domain::nxv_search(query.clone()) {
                    for p in packages {
                        let entry = results_map.entry(p.attribute.clone()).or_insert_with(|| {
                            SearchResult {
                                name: p.attribute.clone(),
                                description: p.description.clone().unwrap_or_default(),
                                versions: Vec::new(),
                                platforms: p.platforms.clone().unwrap_or_default(),
                                is_unfree: false,
                                source_input: None,
                                hash: p.hash.clone(),
                            }
                        });

                        if let Some(license_set) = p.license_set {
                            if license_set.iter().any(|l| l.to_lowercase().contains("unfree")) {
                                entry.is_unfree = true;
                            }
                        }

                        entry.versions.push(ChannelVersion {
                            version: p.version.unwrap_or_else(|| "Unknown".to_string()),
                            channel: "nxv".to_string(),
                            locked_version: None,
                        });
                    }
                }
            } else {
                let mut threads = Vec::new();
                for target in search_targets {
                    let q = query.clone();
                    let t = target.clone();
                    threads.push(std::thread::spawn(move || {
                        let res = domain::nh_search(q, t.clone());
                        (t, res)
                    }));
                }

                for t in threads {
                    if let Ok((channel, Ok(packages))) = t.join() {
                        for p in packages {
                            let entry = results_map.entry(p.attribute.clone()).or_insert_with(|| {
                                SearchResult {
                                    name: p.attribute.clone(),
                                    description: String::new(),
                                    versions: Vec::new(),
                                    platforms: Vec::new(),
                                    is_unfree: false,
                                    source_input: None,
                                    hash: p.hash.clone(),
                                }
                            });

                            if entry.hash.is_none() {
                                entry.hash = p.hash.clone();
                            }

                            if let Some(license_set) = p.license_set {
                                if license_set.iter().any(|l| l.to_lowercase().contains("unfree")) {
                                    entry.is_unfree = true;
                                }
                            }

                            let old_is_unstable = entry
                                .versions
                                .iter()
                                .any(|v| v.channel.contains("unstable"));
                            let new_is_unstable = channel.contains("unstable");

                            let desc = p.description.clone().unwrap_or_default();
                            if entry.description.is_empty()
                                || (new_is_unstable && !old_is_unstable)
                                || desc.len() > entry.description.len()
                            {
                                entry.description = desc;
                            }

                            if let Some(platforms) = p.platforms {
                                for plat in platforms {
                                    if !entry.platforms.contains(&plat) {
                                        entry.platforms.push(plat);
                                    }
                                }
                                entry.platforms.sort();
                            }

                            entry.versions.push(ChannelVersion {
                                version: p.version.unwrap_or_else(|| "Unknown".to_string()),
                                channel: channel.clone(),
                                locked_version: None,
                            });
                        }
                    }
                }
            }

            let mut final_results: Vec<SearchResult> = results_map.into_values().collect();
            let query_lower = query.to_lowercase();
            final_results.sort_by(|a, b| {
                let a_name_lower = a.name.to_lowercase();
                let b_name_lower = b.name.to_lowercase();

                // 1. Exact match on name
                let a_exact = a_name_lower == query_lower;
                let b_exact = b_name_lower == query_lower;
                if a_exact != b_exact {
                    return if a_exact {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                // 2. Starts with query
                let a_starts = a_name_lower.starts_with(&query_lower);
                let b_starts = b_name_lower.starts_with(&query_lower);
                if a_starts != b_starts {
                    return if a_starts {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                // 3. Name contains query
                let a_contains = a_name_lower.contains(&query_lower);
                let b_contains = b_name_lower.contains(&query_lower);
                if a_contains != b_contains {
                    return if a_contains {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                // 4. Description contains query
                let a_desc_contains = a.description.to_lowercase().contains(&query_lower);
                let b_desc_contains = b.description.to_lowercase().contains(&query_lower);
                if a_desc_contains != b_desc_contains {
                    return if a_desc_contains {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                // 5. Alphabetical
                a_name_lower.cmp(&b_name_lower)
            });
            let _ = tx.send(Action::SetPackageSearchResults(Ok(final_results)));
        });
    }

    pub fn check_for_updates(&mut self) {
        if self.mode == Mode::Shell {
            return;
        }

        self.domain.package_updates.clear();
        let packages: Vec<(String, String)> = self.domain.package_info.iter()
            .map(|(name, (_, _, _, source))| (name.clone(), source.clone()))
            .collect();
        
        let inputs = self.domain.inputs.clone();
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            for (pkg_name, source_input) in packages {
                let channel = inputs.iter()
                    .find(|i| i.name == source_input)
                    .map(|i| domain::extract_upstream_channel(i))
                    .unwrap_or_else(|| "nixos-unstable".to_string());

                let tx = tx.clone();
                let pkg = pkg_name.clone();
                std::thread::spawn(move || {
                    if let Ok(results) = domain::nh_search(pkg.clone(), channel) {
                        let latest = results.iter().find(|p| p.attribute == pkg)
                            .or_else(|| results.iter().find(|p| p.attribute.ends_with(&format!(".{}", pkg))));
                        
                        if let Some(latest) = latest {
                            if let Some(version) = &latest.version {
                                let _ = tx.send(Action::UpdatePackageVersion(pkg, version.clone()));
                            }
                        }
                    }
                });
            }
        });
    }

    fn start_fetching_suggestions(&mut self) {
        self.domain.suggestions.is_loading = true;
        Suggestions::fetch_branches(self.tx.clone());
    }

    fn update_suggestions(&mut self) {
        let existing_urls: Vec<String> = self.domain.inputs.iter().map(|i| i.url.clone()).collect();
        self.domain
            .suggestions
            .update_filtered(&self.ui.new_input_name, &existing_urls);
    }
}
