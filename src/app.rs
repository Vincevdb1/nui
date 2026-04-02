use crate::context::{NixFile, find_nix_files};
use crate::nix::{
    Configuration, Input, Package,
    flake::{extract_configurations, extract_inputs},
    suggestions::Suggestions,
};
use crate::components::command_log::LogEntry;
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Deserialize)]
pub struct NHPackage {
    #[serde(rename = "package_attr_name")]
    pub attribute: String,
    #[serde(rename = "package_pname")]
    #[allow(dead_code)]
    pub pname: Option<String>,
    #[serde(rename = "package_pversion")]
    pub version: Option<String>,
    #[serde(rename = "package_description")]
    pub description: Option<String>,
    #[serde(rename = "package_platforms")]
    pub platforms: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelVersion {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub versions: Vec<ChannelVersion>,
    pub platforms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NHSearchResponse {
    results: Vec<NHPackage>,
}

pub fn nh_search(query: String, channel: String) -> Result<Vec<NHPackage>, String> {
    let output = std::process::Command::new("nh")
        .args(["search", "--json", "--platforms", "--channel", &channel, &query])
        .output()
        .map_err(|e| format!("Failed to execute nh: {}", e))?;

    if !output.status.success() {
        return Err(format!("nh search failed with status: {}", output.status));
    }

    let response: NHSearchResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nh output: {}", e))?;

    Ok(response.results)
}

fn extract_channel(url: &str) -> String {
    if url.contains("nixpkgs") {
        for part in url.split('/') {
            if part.starts_with("nixos-") || part.starts_with("nixpkgs-") {
                return part.strip_suffix(".tar.gz").unwrap_or(part).to_string();
            }
        }
    }
    "nixos-unstable".to_string()
}

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
    pub is_adding_package: bool,
    pub package_search_query: String,
    pub last_search_query: String,
    pub package_search_results: Vec<SearchResult>,
    pub package_search_state: ListState,
    pub is_searching_packages: bool,
    pub is_showing_package_details: bool,
    pub searched_channels: Vec<String>,
    pub package_index: HashMap<String, Vec<String>>,
    pub last_search_time: std::time::Instant,
    pub pkg_search_tx: Sender<Result<Vec<SearchResult>, String>>,
    pub pkg_search_rx: Receiver<Result<Vec<SearchResult>, String>>,
    pub new_input_name: String,
    pub new_input_url: String,
    pub input_cursor: usize, // 0 for common inputs, 1 for name, 2 for url
    pub suggestions: Suggestions,
    pub tx: Sender<Vec<(String, String)>>,
    pub rx: Receiver<Vec<(String, String)>>,
    pub pkg_tx: Sender<Result<HashMap<String, (String, String)>, String>>,
    pub pkg_rx: Receiver<Result<HashMap<String, (String, String)>, String>>,
    pub logs: Vec<LogEntry>,
    pub log_rx: Receiver<LogEntry>,
    pub command_log_state: ListState,
    pub throbber_state: throbber_widgets_tui::ThrobberState,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (pkg_tx, pkg_rx) = mpsc::channel();
        let (pkg_search_tx, pkg_search_rx) = mpsc::channel();
        let (log_tx, log_rx) = mpsc::channel();
        crate::components::command_log::init_logger(log_tx);
        crate::command_log("Initializing NUI Application...");
        
        let nix_files = find_nix_files();
        crate::log_output("Filesystem", format!("Found {} nix files", nix_files.len()));

        
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
            is_adding_package: false,
            package_search_query: String::new(),
            last_search_query: String::new(),
            package_search_results: Vec::new(),
            package_search_state: ListState::default(),
            is_searching_packages: false,
            is_showing_package_details: false,
            searched_channels: Vec::new(),
            package_index: HashMap::new(),
            last_search_time: std::time::Instant::now(),
            new_input_name: String::new(),
            new_input_url: String::new(),
            input_cursor: 0,
            suggestions: Suggestions::default(),
            tx,
            rx,
            pkg_tx,
            pkg_rx,
            pkg_search_tx,
            pkg_search_rx,
            logs: Vec::new(),
            log_rx,
            command_log_state: ListState::default(),
            throbber_state: throbber_widgets_tui::ThrobberState::default(),
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
                let total_lines = crate::components::command_log::count_lines(&self.logs);
                if total_lines > 0 {
                    self.command_log_state.select(Some(total_lines - 1));
                }
            }
        }
        if let Ok(branches) = self.rx.try_recv() {
            self.suggestions.all = branches;
            self.update_suggestions();
            self.suggestions.is_loading = false;
        }
        if let Ok(res) = self.pkg_search_rx.try_recv() {
            self.is_searching_packages = false;
            match res {
                Ok(results) => {
                    self.package_search_results = results;
                    if !self.package_search_results.is_empty() && self.package_search_state.selected().is_none() {
                        self.package_search_state.select(Some(0));
                    }
                }
                Err(e) => {
                    crate::log_output("Nix Error", format!("Error searching packages: {}", e));
                }
            }
        }
        if let Ok(res) = self.pkg_rx.try_recv() {
            self.fetching_package_details = false;
            self.pending_fetches.clear();
            match res {
                Ok(results) => {
                    crate::log_output("Nix Output", format!("Successfully fetched {} package details", results.len()));
                    self.package_fetch_error = None;
                    for (name, details) in results {
                        self.package_info.insert(name, details);
                    }
                }
                Err(e) => {
                    crate::log_output("Nix Error", format!("Error fetching package details: {}", e));
                    self.package_fetch_error = Some(e);
                }
            }
        }
    }

    pub fn fetch_package_details_from_config(&mut self) {
        if let Some(config) = self.configurations.get(self.selected_configuration_index) {
            let config_type = config.config_type.clone();
            let config_name = config.path.clone();

            crate::log_action(
                format!("Fetching package details for {}", config_name),
                format!("Config Type: {}", config_type)
            );

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
        self.throbber_state.calc_next();

        if self.is_adding_package && self.last_search_time.elapsed() > std::time::Duration::from_millis(300) {
            let query = self.package_search_query.clone();
            if query != self.last_search_query {
                self.last_search_query = query.clone();
                if query.is_empty() {
                    self.package_search_results.clear();
                } else if !self.is_searching_packages {
                    self.perform_package_search();
                }
            }
        }
    }

    pub fn next_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 5,
            5 => 1,
            _ => 1,
        };
    }

    pub fn previous_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 5,
            2 => 1,
            3 => 2,
            4 => 3,
            5 => 4,
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
        crate::log_action(
            format!("Adding input: {}", self.new_input_name),
            format!("Target: {:?}\nURL: {}", path, self.new_input_url)
        );
        let new_content =
            crate::nix::flake::add_input(&content, &self.new_input_name, &self.new_input_url);
        if let Err(e) = std::fs::write(&path, new_content) {
            crate::log_output("Filesystem Error", format!("Failed to write {:?}: {}", path, e));
            eprintln!("Failed to write {:?}: {}", path, e);
        } else {
            crate::log_output("Filesystem Output", format!("Successfully updated {:?}", path));
        }

        self.is_adding_input = false;
        self.new_input_name.clear();
        self.new_input_url.clear();
        self.input_cursor = 0;
    }

    pub fn perform_package_search(&mut self) {
        if self.package_search_query.is_empty() {
            self.package_search_results.clear();
            return;
        }

        self.is_searching_packages = true;
        let query = self.package_search_query.clone();
        let tx = self.pkg_search_tx.clone();
        
        // Extract channels from inputs
        let mut channels: Vec<String> = self.inputs.iter()
            .filter(|i| i.url.contains("nixpkgs"))
            .map(|i| extract_channel(&i.url))
            .collect();
            
        if channels.is_empty() {
            channels.push("nixos-unstable".to_string());
        }
        
        channels.sort();
        channels.dedup();
        self.searched_channels = channels.clone();

        std::thread::spawn(move || {
            crate::log_action(
                format!("Searching across {} channels for {}", channels.len(), query),
                format!("nh search --json (channels: {})", channels.join(", "))
            );

            let mut threads = Vec::new();
            for channel in channels {
                let q = query.clone();
                let ch = channel.clone();
                threads.push(std::thread::spawn(move || {
                    let res = nh_search(q, ch.clone());
                    (ch, res)
                }));
            }

            let mut results_map: HashMap<String, SearchResult> = HashMap::new();
            for t in threads {
                match t.join() {
                    Ok((channel, Ok(packages))) => {
                        for p in packages {
                            let entry = results_map.entry(p.attribute.clone()).or_insert_with(|| SearchResult {
                                name: p.attribute.clone(),
                                description: String::new(),
                                versions: Vec::new(),
                                platforms: Vec::new(),
                            });
                            
                            // Prefer description from unstable channel, or whichever is longer
                            let old_is_unstable = entry.versions.iter().any(|v| v.channel.contains("unstable"));
                            let new_is_unstable = channel.contains("unstable");
                            
                            let desc = p.description.clone().unwrap_or_default();
                            if entry.description.is_empty() 
                                || (new_is_unstable && !old_is_unstable)
                                || desc.len() > entry.description.len() {
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
                            });
                        }
                    }
                    Ok((channel, Err(e))) => {
                        crate::log_output("Nix Error", format!("Error searching channel {}: {}", channel, e));
                    }
                    Err(_) => {
                        crate::log_output("Nix Error", "Search thread panicked".to_string());
                    }
                }
            }

            let mut final_results: Vec<SearchResult> = results_map.into_values().collect();
            final_results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            let _ = tx.send(Ok(final_results));
        });
    }

    pub fn add_package(&mut self) {
        if let Some(result) = self.package_search_results.get(
            self.package_search_state.selected().unwrap_or(0)
        ) {
            let package_name = &result.name;
            let _path = if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
                file.path.clone()
            } else {
                std::path::PathBuf::from("flake.nix")
            };

            let config = if let Some(c) = self.configurations.get(self.selected_configuration_index) {
                c
            } else {
                return;
            };

            crate::log_action(
                format!("Adding package: {}", package_name),
                format!("Target Config: {} ({})", config.path, config.config_type)
            );

            // This is a VERY simplified add_package.
            // In a real scenario, we'd need to find where to insert in the AST.
            // For now, let's try a very basic approach or use nix-editor if possible.
            // nix-editor doesn't easily support adding to lists in nested sets.
            
            // We'll leave the actual file modification as a placeholder or attempt a basic one.
            crate::log_output("Filesystem", format!("Requested to add {} to {}", package_name, config.path));
            
            self.is_adding_package = false;
            self.package_search_query.clear();
            self.package_search_results.clear();
            self.fetch_package_details_from_config();
        }
    }
}
