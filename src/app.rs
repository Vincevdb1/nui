use crate::context::{NixFile, find_nix_files};
use crate::nix::{
    Configuration, Input, Package,
    flake::{extract_configurations, extract_inputs},
    suggestions::Suggestions,
};
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
    pub selected_configuration_index: usize,
    pub inputs: Vec<Input>,
    pub configurations: Vec<Configuration>,
    pub package_info: HashMap<String, (String, String)>,
    pub pending_fetches: HashSet<String>,
    pub fetching_package_details: bool,
    pub package_fetch_error: Option<String>,
    pub is_adding_input: bool,
    pub new_input_name: String,
    pub new_input_url: String,
    pub input_cursor: usize, // 0 for common inputs, 1 for name, 2 for url
    pub suggestions: Suggestions,
    pub tx: Sender<Vec<(String, String)>>,
    pub rx: Receiver<Vec<(String, String)>>,
    pub pkg_tx: Sender<Result<HashMap<String, (String, String)>, String>>,
    pub pkg_rx: Receiver<Result<HashMap<String, (String, String)>, String>>,
    pub logs: Vec<String>,
    pub log_rx: Receiver<String>,
    pub command_log_state: ListState,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (pkg_tx, pkg_rx) = mpsc::channel();
        let (log_tx, log_rx) = mpsc::channel();
        crate::components::command_log::init_logger(log_tx);
        crate::command_log("Initializing NUI Application...");
        
        let nix_files = find_nix_files();
        crate::command_log(format!("Found {} nix files", nix_files.len()));

        
        let (inputs, configurations) = if let Some(file) = nix_files.first() {
            let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
            (extract_inputs(&flake_content), extract_configurations(&flake_content))
        } else {
            (Vec::new(), Vec::new())
        };

        let mut app = Self {
            should_quit: false,
            selected_index: 2,
            nix_files,
            selected_nix_file_index: 0,
            selected_configuration_index: 0,
            inputs,
            configurations,
            package_info: HashMap::new(),
            pending_fetches: HashSet::new(),
            fetching_package_details: false,
            package_fetch_error: None,
            is_adding_input: false,
            new_input_name: String::new(),
            new_input_url: String::new(),
            input_cursor: 0,
            suggestions: Suggestions::default(),
            tx,
            rx,
            pkg_tx,
            pkg_rx,
            logs: Vec::new(),
            log_rx,
            command_log_state: ListState::default(),
        };
        app.fetch_package_details_from_config();
        app
    }

    pub fn process_suggestions(&mut self) {
        let mut new_logs = false;
        while let Ok(msg) = self.log_rx.try_recv() {
            self.logs.push(msg);
            new_logs = true;
        }
        if new_logs {
            if !self.logs.is_empty() {
                self.command_log_state.select(Some(self.logs.len() - 1));
            }
        }
        if let Ok(branches) = self.rx.try_recv() {
            crate::command_log("Successfully fetched nixpkgs branches");
            self.suggestions.all = branches;
            self.update_suggestions();
            self.suggestions.is_loading = false;
        }
        if let Ok(res) = self.pkg_rx.try_recv() {
            self.fetching_package_details = false;
            self.pending_fetches.clear();
            match res {
                Ok(results) => {
                    crate::command_log(format!("Successfully fetched {} package details", results.len()));
                    self.package_fetch_error = None;
                    for (name, details) in results {
                        self.package_info.insert(name, details);
                    }
                }
                Err(e) => {
                    crate::command_log(format!("Error fetching package details: {}", e));
                    self.package_fetch_error = Some(e);
                }
            }
        }
    }

    pub fn fetch_package_details_from_config(&mut self) {
        if let Some(config) = self.configurations.get(self.selected_configuration_index) {
            let config_type = config.config_type.clone();
            let config_name = config.path.clone();

            crate::command_log(format!("Fetching package details for {} ({})", config_name, config_type));

            let flake_path = if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
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

            self.fetching_package_details = true;
            self.package_fetch_error = None;
            self.package_info.clear();
            self.pending_fetches.clear();
            // We don't know the exact package names yet without evaluating,
            // but we can mark the app as "fetching" in some way if we wanted.
            
            let tx = self.pkg_tx.clone();
            std::thread::spawn(move || {
                let res = Package::fetch_from_config(&flake_path, &config_type, &config_name);
                let _ = tx.send(res);
            });
        }
    }

    pub fn update_suggestions(&mut self) {
        let existing_urls: Vec<String> = self.inputs.iter().map(|i| i.url.clone()).collect();
        self.suggestions
            .update_filtered(&self.new_input_name, &existing_urls);
    }

    pub fn start_fetching(&mut self) {
        self.suggestions.is_loading = true;
        Suggestions::fetch_branches(self.tx.clone());
    }

    pub fn tick(&mut self) {
        self.process_suggestions();
    }

    pub fn next_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 1,
            _ => 1,
        };
    }

    pub fn previous_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 4,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => 1,
        };
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn update_context(&mut self) {
        if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
            let content = std::fs::read_to_string(&file.path).unwrap_or_default();
            self.inputs = extract_inputs(&content);
            self.configurations = extract_configurations(&content);
            if self.selected_configuration_index >= self.configurations.len() {
                self.selected_configuration_index = 0;
            }
            self.fetch_package_details_from_config();
        }
    }

    pub fn add_input(&mut self) {
        let path = if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
            file.path.clone()
        } else {
            std::path::PathBuf::from("flake.nix")
        };

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        crate::command_log(format!("Adding new input: {} ({}) to {:?}", self.new_input_name, self.new_input_url, path));
        let new_content =
            crate::nix::flake::add_input(&content, &self.new_input_name, &self.new_input_url);
        if let Err(e) = std::fs::write(&path, new_content) {
            eprintln!("Failed to write {:?}: {}", path, e);
        }

        self.update_context();

        self.is_adding_input = false;
        self.new_input_name.clear();
        self.new_input_url.clear();
        self.input_cursor = 0;
    }
}
