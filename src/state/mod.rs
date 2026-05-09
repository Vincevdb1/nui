pub mod domain;
pub mod ui;

pub use domain::{ChannelVersion, DomainData, SearchResult};
pub use ui::UiState;

use crate::action::Action;
use crate::context::find_nix_files;
use crate::nix::flake::{extract_inputs, fetch_outputs};
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

        let (nix_files, inputs, outputs) = if mode == Mode::Flake {
            let nix_files = find_nix_files();
            crate::log_output("Filesystem", format!("Found {} nix files", nix_files.len()));

            let (inputs, outputs) = if let Some(file) = nix_files.first() {
                let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
                let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                let lock_content = std::fs::read_to_string(lock_path).ok();
                (
                    extract_inputs(&flake_content, lock_content.as_deref()),
                    fetch_outputs(file.path.parent().unwrap_or(std::path::Path::new(".")))
                        .unwrap_or_default(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            (nix_files, inputs, outputs)
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
                outputs,
                nix_search_cli_version: domain::get_tool_version("nix-search"),
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
                    self.domain.outputs = Vec::new();
                } else {
                    self.mode = Mode::Flake;
                    self.ui.selected_index = 1;
                    if !self.domain.package_info.is_empty() {
                        self.ui.package_table_state.select(Some(1));
                    }
                    // Re-initialize Flake mode data
                    let nix_files = find_nix_files();
                    self.domain.nix_files = nix_files;
                    if let Some(file) = self.domain.nix_files.first() {
                        let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
                        let lock_path = file.path.parent().unwrap_or(std::path::Path::new(".")).join("flake.lock");
                        let lock_content = std::fs::read_to_string(lock_path).ok();
                        self.domain.inputs = extract_inputs(&flake_content, lock_content.as_deref());
                        self.domain.outputs = fetch_outputs(
                            file.path.parent().unwrap_or(std::path::Path::new(".")),
                        )
                        .unwrap_or_default();
                        self.fetch_package_details();
                    }
                }
            }
            Action::MoveDown => match self.ui.selected_index {
                1 => {
                    if self.mode == Mode::Shell && !self.shell_packages.is_empty() {
                        let i = match self.ui.shell_package_list_state.selected() {
                            Some(i) => {
                                if i >= self.shell_packages.len() {
                                    1
                                } else {
                                    i + 1
                                }
                            }
                            None => 1,
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
                3 => {
                    self.update(Action::MoveInputSelectionDown);
                }
                4 => {
                    if !self.domain.outputs.is_empty() {
                        self.ui.selected_output_index =
                            (self.ui.selected_output_index + 1)
                                % self.domain.outputs.len();
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
                                if i <= 1 {
                                    self.shell_packages.len()
                                } else {
                                    i - 1
                                }
                            }
                            None => 1,
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
                3 => {
                    self.update(Action::MoveInputSelectionUp);
                }
                4 => {
                    if !self.domain.outputs.is_empty() {
                        self.ui.selected_output_index =
                            if self.ui.selected_output_index == 0 {
                                self.domain.outputs.len() - 1
                            } else {
                                self.ui.selected_output_index - 1
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
            Action::MoveInputSelectionDown => {
                let count = self.domain.inputs.len();
                if count > 0 {
                    let i = match self.ui.input_table_state.selected() {
                        Some(i) => {
                            if i >= count {
                                1
                            } else {
                                i + 1
                            }
                        }
                        None => 1,
                    };
                    self.ui.input_table_state.select(Some(i));
                }
            }
            Action::MoveInputSelectionUp => {
                let count = self.domain.inputs.len();
                if count > 0 {
                    let i = match self.ui.input_table_state.selected() {
                        Some(i) => {
                            if i <= 1 {
                                count
                            } else {
                                i - 1
                            }
                        }
                        None => 1,
                    };
                    self.ui.input_table_state.select(Some(i));
                }
            }
            Action::RemoveInput(input_name) => {
                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let flake_path = file.path.clone();
                    let tx = self.tx.clone();
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
                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let flake_path = file.path.clone();
                    if let Some(output) = self
                        .domain
                        .outputs
                        .get(self.ui.selected_output_index)
                    {
                        let parts: Vec<&str> = output.path.split('.').collect();
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
                self.ui.input_cursor = 1;
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
                            let version = result.versions.first().map(|v| v.version.clone()).unwrap_or_else(|| "Unknown".to_string());
                            let pkg_to_add = if let Some(hash) = &result.hash {
                                format!("nixpkgs/{}#{}@{}", hash, package_name, version)
                            } else {
                                format!("{}@{}", package_name, version)
                            };

                            if !self.shell_packages.contains(&pkg_to_add) {
                                self.shell_packages.push(pkg_to_add);
                            }
                            self.ui.is_adding_package = false;
                        } else {
                            if let Some(file) =
                                self.domain.nix_files.get(self.ui.selected_nix_file_index)
                            {
                                let flake_path = file.path.clone();
                                if let Some(output) = self
                                    .domain
                                    .outputs
                                    .get(self.ui.selected_output_index)
                                {
                                    let parts: Vec<&str> = output.path.split('.').collect();
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
                let inputs_len = if self.mode == Mode::Flake {
                    if let Some(pkg_name) = &self.ui.selected_package_name {
                        domain::get_available_inputs(&self.domain.inputs, pkg_name, &self.domain.package_search_results).len()
                    } else {
                        0
                    }
                } else {
                    0
                };
                let total = inputs_len + self.domain.package_versions.len();

                if total > 0 {
                    let i = match self.ui.version_list_state.selected() {
                        Some(i) => {
                            if i >= total.saturating_sub(1) {
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
                let inputs_len = if self.mode == Mode::Flake {
                    if let Some(pkg_name) = &self.ui.selected_package_name {
                        domain::get_available_inputs(&self.domain.inputs, pkg_name, &self.domain.package_search_results).len()
                    } else {
                        0
                    }
                } else {
                    0
                };
                let total = inputs_len + self.domain.package_versions.len();

                if total > 0 {
                    let i = match self.ui.version_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                total.saturating_sub(1)
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
                let name = self.ui.new_input_name.clone();
                let url = self.ui.new_input_url.clone();

                if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                    let flake_path = file.path.clone();
                    let pkg_name = self.ui.selected_package_name.clone();
                    let selected_output_index = self.ui.selected_output_index;
                    let outputs = self.domain.outputs.clone();
                    let tx = self.tx.clone();

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

                                // We need to write the content with the new input first so add_package can find it (if it uses nix-editor)
                                // Actually, add_package reads from file. This is a bit inefficient but safe.
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

                self.ui.is_adding_input = false;
                self.ui.selected_package_name = None;
                self.ui.selected_version = None;
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
            Action::SetSuggestions(res) => {
                self.domain.suggestions.is_loading = false;
                match res {
                    Ok(suggestions) => {
                        self.domain.suggestions.all = suggestions;
                        self.domain.suggestions.error = None;
                        self.update_suggestions();
                    }
                    Err(e) => {
                        self.domain.suggestions.error = Some(e);
                    }
                }
            }
            Action::SetPackageSearchResults(res) => {
                self.ui.is_searching_packages = false;
                match res {
                    Ok(results) => {
                        self.ui.package_search_error = None;
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
                            self.ui.package_search_state.select(None);
                        }
                    }
                    Err(e) => {
                        self.ui.package_search_error = Some(e.clone());
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
            Action::SetPackageDetails(fetch_id, res) => {
                if fetch_id != self.ui.package_fetch_id {
                    return;
                }
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
                        if let Some(output) = self
                            .domain
                            .outputs
                            .get(self.ui.selected_output_index)
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
                                // Fallback: check if any attribute starts with this name (pname case)
                                source_map.iter()
                                    .find(|(k, _)| name.starts_with(*k) || k.starts_with(&name))
                                    .map(|(_, v)| v.clone())
                                    .unwrap_or_default()
                            });
                            
                            // Only add if we found a source OR it has a non-empty version
                            // This helps filter out internal hooks that evaluation might pull in
                            if !source.is_empty() || !ver.is_empty() {
                                self.domain
                                    .package_info
                                    .insert(name, (desc, ver, unfree, source));
                            }
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
                    match fetch_outputs(
                        file.path.parent().unwrap_or(std::path::Path::new(".")),
                    ) {
                        Ok(outputs) => self.domain.outputs = outputs,
                        Err(e) => {
                            crate::log_output("Nix Error", format!("Failed to fetch outputs: {}", e));
                            self.domain.outputs = Vec::new();
                        }
                    }
                    if self.ui.selected_output_index >= self.domain.outputs.len() {
                        self.ui.selected_output_index = 0;
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
                            self.shell_packages.len()
                        } else {
                            index + 1
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
                        let pinned_pkg = format!("nixpkgs/{}#{}@{}", version_info.hash, pkg_name, version_info.version);
                        self.shell_packages.push(pinned_pkg);
                    }
                    self.ui.is_adding_package = false;
                    self.ui.is_selecting_version = false;
                    self.ui.selected_package_name = None;
                    return;
                }

                if let Some(_pkg_name) = self.ui.selected_package_name.clone() {
                    self.ui.selected_version = Some(version_info.clone());
                    self.ui.is_adding_input = true;
                    self.ui.is_adding_package = false;
                    self.ui.is_selecting_version = false;
                    self.ui.new_input_name = format!("nixpkgs-{}", &version_info.hash[..7]);
                    self.ui.new_input_url = format!("github:NixOS/nixpkgs/{}", version_info.hash);
                    self.ui.input_cursor = 1;
                }
            }
            Action::SelectInputForPackage(input_name) => {
                if self.mode == Mode::Shell {
                    if let Some(pkg_name) = self.ui.selected_package_name.clone() {
                        let pkg = format!("{}#{}", input_name, pkg_name);
                        self.shell_packages.push(pkg);
                    }
                    self.ui.is_adding_package = false;
                    self.ui.is_selecting_version = false;
                    self.ui.selected_package_name = None;
                    return;
                }

                if let Some(pkg_name) = self.ui.selected_package_name.clone() {
                    if let Some(file) = self.domain.nix_files.get(self.ui.selected_nix_file_index) {
                        let flake_path = file.path.clone();
                        let selected_output_index = self.ui.selected_output_index;
                        let outputs = self.domain.outputs.clone();
                        let tx = self.tx.clone();

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
                                    crate::log_output("Nix", format!("Added package {} to output {}", prefixed_pkg, output.path));
                                }
                                let _ = tx.send(Action::RefreshContext);
                            }
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
        if let Some(output) = self
            .domain
            .outputs
            .get(self.ui.selected_output_index)
        {
            let output_type = output.config_type.clone();
            let output_name = output.path.clone();

            crate::log_action(
                format!("Fetching package details for {}", output_name),
                format!("Output Type: {}", output_type),
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
            self.ui.package_fetch_id += 1;
            let fetch_id = self.ui.package_fetch_id;

            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let res =
                    crate::nix::Package::fetch_from_output(&flake_path, &output_type, &output_name);
                let _ = tx.send(Action::SetPackageDetails(fetch_id, res));
            });
        }
    }

    fn perform_package_search(&mut self) {
        if self.ui.package_search_query.is_empty() {
            self.domain.package_search_results.clear();
            return;
        }

        self.ui.is_searching_packages = true;
        self.ui.package_search_error = None;
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

        let mut non_nixpkgs_inputs = Vec::new();
        if self.mode == Mode::Flake {
            for input in &self.domain.inputs {
                if !input.url.contains("nixpkgs") {
                    non_nixpkgs_inputs.push(input.clone());
                }
            }
        }

        search_targets.sort();
        search_targets.dedup();
        self.domain.searched_channels = search_targets.clone();

        let mode = self.mode.clone();
        let non_nixpkgs_names: Vec<String> = non_nixpkgs_inputs.iter().map(|i| i.name.clone()).collect();

        std::thread::spawn(move || {
            let mut results_map: HashMap<String, SearchResult> = HashMap::new();

            if mode == Mode::Shell {
                let mut nix_search_err = None;
                let mut nxv_search_err = None;

                // Nix Search (current unstable channel)
                match domain::nix_search_cli(query.clone(), "nixos-unstable".to_string()) {
                    Ok(packages) => {
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
                                channel: "nixos-unstable".to_string(),
                                locked_version: None,
                            });
                        }
                    }
                    Err(e) => {
                        crate::log_output("Nix Search Error", e.clone());
                        nix_search_err = Some(e);
                    }
                }

                // NXV Search (for version history and additional results)
                match domain::nxv_search(query.clone()) {
                    Ok(packages) => {
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

                            if entry.description.is_empty() {
                                if let Some(desc) = p.description {
                                    entry.description = desc;
                                }
                            }

                            if entry.platforms.is_empty() {
                                if let Some(platforms) = p.platforms {
                                    entry.platforms = platforms;
                                }
                            }

                            if entry.hash.is_none() {
                                entry.hash = p.hash.clone();
                            }

                            if let Some(license_set) = p.license_set {
                                if license_set.iter().any(|l| l.to_lowercase().contains("unfree")) {
                                    entry.is_unfree = true;
                                }
                            }

                            // Only add nxv version if not already present or as a distinct "nxv" entry
                            // Typically NXV provides history, but here we just want to show it's indexed
                            if !entry.versions.iter().any(|v| v.channel == "nxv") {
                                entry.versions.push(ChannelVersion {
                                    version: p.version.unwrap_or_else(|| "Unknown".to_string()),
                                    channel: "nxv".to_string(),
                                    locked_version: None,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        crate::log_output("NXV Search Error", e.clone());
                        nxv_search_err = Some(e);
                    }
                }

                if results_map.is_empty() {
                    if let Some(e) = nix_search_err {
                        let _ = tx.send(Action::SetPackageSearchResults(Err(e)));
                        return;
                    }
                    if let Some(e) = nxv_search_err {
                        let _ = tx.send(Action::SetPackageSearchResults(Err(e)));
                        return;
                    }
                }
            } else {
                let mut threads = Vec::new();
                for target in search_targets {
                    let q = query.clone();
                    let t = target.clone();
                    threads.push(std::thread::spawn(move || {
                        let res = domain::nix_search_cli(q, t.clone());
                        (t, res)
                    }));
                }

                for input in non_nixpkgs_inputs {
                    let q = query.clone();
                    let input_name = input.name.clone();
                    let input_url = input.url.clone();
                    threads.push(std::thread::spawn(move || {
                        let res = domain::nix_search_flake(input_url, q);
                        (input_name, res)
                    }));
                }

                // Search current flake
                let q = query.clone();
                threads.push(std::thread::spawn(move || {
                    let res = domain::nix_search_flake(".".to_string(), q);
                    ("self".to_string(), res)
                }));

                let mut search_errors = Vec::new();
                for t in threads {
                    match t.join() {
                        Ok((source, Ok(packages))) => {
                            let is_input = non_nixpkgs_names.contains(&source) || source == "self";
                            for p in packages {
                                // Make name unique for non-nixpkgs inputs to avoid collisions
                                let key = if is_input {
                                    format!("{}.{}", source, p.attribute)
                                } else {
                                    p.attribute.clone()
                                };

                                let entry = results_map.entry(key.clone()).or_insert_with(|| {
                                    SearchResult {
                                        name: key.clone(),
                                        description: String::new(),
                                        versions: Vec::new(),
                                        platforms: Vec::new(),
                                        is_unfree: false,
                                        source_input: if is_input { Some(source.clone()) } else { None },
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
                                let new_is_unstable = source.contains("unstable");

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
                                    channel: source.clone(),
                                    locked_version: None,
                                });
                            }
                        }
                        Ok((source, Err(e))) => {
                            crate::log_output(format!("Search Error ({})", source), e.clone());
                            search_errors.push(e);
                        }
                        Err(_) => {
                            crate::log_output("Thread Error", "A search thread panicked");
                        }
                    }
                }

                if results_map.is_empty() && !search_errors.is_empty() {
                    let _ = tx.send(Action::SetPackageSearchResults(Err(search_errors[0].clone())));
                    return;
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
                    match domain::nix_search_cli(pkg.clone(), channel) {
                        Ok(results) => {
                            let latest = results.iter().find(|p| p.attribute == pkg)
                                .or_else(|| results.iter().find(|p| p.attribute.ends_with(&format!(".{}", pkg))));
                            
                            if let Some(latest) = latest {
                                if let Some(version) = &latest.version {
                                    let _ = tx.send(Action::UpdatePackageVersion(pkg, version.clone()));
                                }
                            }
                        }
                        Err(e) => {
                            crate::log_output(format!("Update Check Error ({})", pkg), e);
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
