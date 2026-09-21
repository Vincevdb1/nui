pub mod domain;
pub mod ui;

pub use domain::DomainData;
pub use ui::UiState;

use crate::action::Action;
use crate::services::NixService;
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

    pub tx: Sender<Action>,
    pub rx: Receiver<Action>,
    pub nix_service: NixService,
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

        let nix_service = NixService::new(tx.clone());

        let (nix_files, inputs, outputs) = if mode == Mode::Flake {
            nix_service.get_initial_context()
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        let (system_version, system_path, system_hash) = nix_service.get_system_info();
        let (nix_search_version, nxv_version) = nix_service.get_tool_versions();

        let mut app = Self {
            mode,
            shell_packages,
            ui: UiState::default(),
            domain: DomainData {
                nix_files,
                inputs,
                outputs,
                nix_search_cli_version: nix_search_version,
                nxv_version,
                system_nixpkgs_version: system_version,
                system_nixpkgs_path: system_path,
                system_nixpkgs_hash: system_hash,
                ..Default::default()
            },
            should_quit: false,
            tx: tx.clone(),
            rx,
            nix_service,
        };
        app.ui.templates = app.nix_service.load_templates();

        if app.mode == Mode::Shell {
            app.ui.selected_index = 1;
        }

        if app.mode == Mode::Flake {
            app.fetch_package_details();
        }

        if app.domain.nxv_version.is_some() {
            crate::log_output("NXV", "Starting background update check...");
            app.nix_service.start_nxv_update_check();
        }

        app
    }

    pub fn process_background_results(&mut self) {
        while let Ok(action) = self.rx.try_recv() {
            self.update(action);
        }
    }

    pub fn fetch_package_details(&mut self) {
        if let Some(output) = self.domain.outputs.get(self.ui.selected_output_index) {
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

            self.nix_service
                .fetch_package_details(flake_path, output_type, output_name, fetch_id);
        }
    }

    pub fn cancel_package_search(&mut self) {
        self.ui.package_search_id += 1;
        self.ui.is_searching_packages = false;
        crate::state::domain::cancel_registered(&self.nix_service.search_children());
    }

    pub fn perform_package_search(&mut self) {
        if self.ui.package_search_query.is_empty() {
            self.domain.package_search_results.clear();
            return;
        }

        self.ui.is_searching_packages = true;
        self.ui.package_search_error = None;
        self.ui.package_search_id += 1;
        let search_id = self.ui.package_search_id;
        let query = self.ui.package_search_query.clone();

        let mut search_targets: Vec<String> = if self.mode == Mode::Shell {
            let mut targets = vec!["nxv".to_string()];
            if let Some(version) = &self.domain.system_nixpkgs_version {
                targets.push(version.clone());
            } else {
                targets.push("nixos-unstable".to_string());
            }
            targets
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
        self.domain.searched_channels = search_targets;

        self.nix_service.perform_package_search(
            search_id,
            query,
            self.mode.clone(),
            self.domain.inputs.clone(),
            self.domain.system_nixpkgs_hash.clone(),
            self.domain.system_nixpkgs_version.clone(),
        );
    }

    pub fn check_for_updates(&mut self) {
        if self.mode == Mode::Shell {
            return;
        }

        self.domain.package_updates.clear();
        let packages: Vec<(String, String)> = self
            .domain
            .package_info
            .iter()
            .map(|(name, (_, _, _, source))| (name.clone(), source.clone()))
            .collect();

        self.nix_service
            .check_for_updates(packages, self.domain.inputs.clone());
    }

    pub fn start_fetching_suggestions(&mut self) {
        self.domain.suggestions.is_loading = true;
        self.nix_service.start_fetching_suggestions();
    }

    pub fn update_suggestions(&mut self) {
        let existing_urls: Vec<String> = self.domain.inputs.iter().map(|i| i.url.clone()).collect();
        self.domain
            .suggestions
            .update_filtered(&self.ui.new_input_name, &existing_urls);
    }

    pub fn apply_template_logic(&mut self, template_name: String) {
        if let Some(config_dir) = dirs::config_dir() {
            let template_path = config_dir
                .join("nui")
                .join("templates")
                .join(&template_name);
            crate::log_output(
                "Debug",
                format!(
                    "Applying template: {} (Mode: {:?})",
                    template_name, self.mode
                ),
            );

            if template_path.exists() {
                self.nix_service
                    .apply_template(template_path, self.mode.clone());
            } else {
                crate::log_output("Error", format!("Template not found: {:?}", template_path));
            }
        }
    }

    pub fn fetch_shell_package_metadata(&mut self, pkg_id: String) {
        self.nix_service.fetch_shell_package_metadata(pkg_id);
    }

    pub fn update(&mut self, action: Action) {
        let context = crate::context::Context::new(self.tx.clone());
        crate::handlers::handle_action(self, &context, action);
    }
}
