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

    pub tx: Sender<Action>,
    pub rx: Receiver<Action>,
}

pub fn load_templates() -> Vec<(String, String)> {
    let mut templates = Vec::new();
    if let Some(config_dir) = dirs::config_dir() {
        let template_dir = config_dir.join("nui").join("templates");
        if let Ok(entries) = std::fs::read_dir(template_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        if let Some(name) = entry.file_name().to_str() {
                            let description = if let Ok(content) = std::fs::read_to_string(&path) {
                                content.lines()
                                    .find(|l| l.trim().starts_with("description"))
                                    .and_then(|l| l.split('"').nth(1))
                                    .unwrap_or("No description")
                                    .to_string()
                            } else {
                                "No description".to_string()
                            };
                            templates.push((name.to_string(), description));
                        }
                    }
                }
            }
        }
    }
    templates
}

fn extract_packages_from_template(content: &str) -> Vec<String> {
    let mut packages = Vec::new();
    
    let patterns = ["packages = [", "buildInputs = [", "nativeBuildInputs = ["];
    
    for pattern in patterns {
        let mut current_pos = 0;
        while let Some(start) = content[current_pos..].find(pattern) {
            let actual_start = current_pos + start + pattern.len();
            let rest = &content[actual_start..];
            if let Some(end) = rest.find("];") {
                let list = &rest[..end];
                for item in list.split_whitespace() {
                    let mut pkg = item;
                    if let Some(stripped) = pkg.strip_prefix("pkgs.") {
                        pkg = stripped;
                    }
                    let clean_pkg = pkg.trim_matches(|c| c == '"' || c == '\'' || c == ';' || c == '[' || c == ']' || c == '(' || c == ')');
                    
                    if !clean_pkg.is_empty() 
                       && clean_pkg != "with" 
                       && clean_pkg != "pkgs" 
                       && !clean_pkg.starts_with("self.")
                       && !clean_pkg.contains("${") 
                    {
                        if !packages.contains(&clean_pkg.to_string()) {
                            packages.push(clean_pkg.to_string());
                        }
                    }
                }
                current_pos = actual_start + end + 2;
            } else {
                break;
            }
        }
    }
    packages
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
                system_nixpkgs_version: std::process::Command::new("nix-instantiate")
                    .args(["--eval", "-E", "(import <nixpkgs> {}).lib.version", "--json"])
                    .output()
                    .ok()
                    .and_then(|o| if o.status.success() {
                        serde_json::from_slice::<String>(&o.stdout).ok()
                    } else {
                        None
                    }),
                system_nixpkgs_path: std::process::Command::new("nix")
                    .args(["eval", "--raw", "--impure", "--expr", "toString <nixpkgs>"])
                    .output()
                    .ok()
                    .and_then(|o| if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    } else {
                        None
                    }),
                system_nixpkgs_hash: std::process::Command::new("nix")
                    .args(["eval", "--raw", "--impure", "--expr", "builtins.substring 0 32 (builtins.baseNameOf (toString <nixpkgs>))"])
                    .output()
                    .ok()
                    .and_then(|o| if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    } else {
                        None
                    }),
                ..Default::default()
            },
            should_quit: false,
            tx,
            rx,
        };
        app.ui.templates = load_templates();

        if app.mode == Mode::Shell {
            app.ui.selected_index = 1;
        }

        if app.mode == Mode::Flake {
            app.fetch_package_details();
        }

        if app.domain.nxv_version.is_some() {
            crate::log_output("NXV", "Starting background update check...");
            let tx = app.tx.clone();
            std::thread::spawn(move || {
                use std::io::{BufReader, Read};
                use std::process::{Command, Stdio};

                let mut child = match Command::new("script")
                    .args(["-q", "-c", "nxv update", "/dev/null"])
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    Ok(child) => child,
                    Err(e) => {
                        crate::log_output("NXV Error", format!("Failed to spawn nxv update: {}", e));
                        return;
                    }
                };

                let stdout = child.stdout.take().unwrap();
                let mut reader = BufReader::new(stdout);
                let mut buffer = Vec::new();
                let mut b = [0u8; 1];

                fn strip_ansi(s: &str) -> String {
                    let mut result = String::new();
                    let mut iter = s.chars();
                    while let Some(c) = iter.next() {
                        if c == '\x1b' {
                            if let Some('[') = iter.next() {
                                while let Some(c2) = iter.next() {
                                    if (0x40..=0x7e).contains(&(c2 as u8)) {
                                        break;
                                    }
                                }
                            }
                        } else {
                            result.push(c);
                        }
                    }
                    result
                }

                while reader.read_exact(&mut b).is_ok() {
                    if b[0] == b'\n' || b[0] == b'\r' {
                        let line = String::from_utf8_lossy(&buffer);
                        let clean_line = strip_ansi(&line);
                        let trimmed = clean_line.trim();
                        if !trimmed.is_empty() {
                            let display = if let Some(start) = trimmed.find('[') {
                                if let Some(end) = trimmed.find(']') {
                                    let stats = trimmed[end + 1..].trim();
                                    let bar_content = &trimmed[start + 1..end];
                                    let filled_count = bar_content.chars().filter(|&c| !c.is_whitespace() && c != '-').count();
                                    let total_chars = bar_content.chars().count();
                                    let ratio = if total_chars > 0 { filled_count as f32 / total_chars as f32 } else { 0.0 };
                                    let filled_segments = (ratio * 10.0).round() as usize;
                                    let bar = format!("[{}{}]", "█".repeat(filled_segments), " ".repeat(10 - filled_segments));
                                    format!("{} {}", bar, stats)
                                } else {
                                    trimmed[start..].to_string()
                                }
                            } else {
                                trimmed.to_string()
                            };
                            let _ = tx.send(Action::UpdateNxvProgress(Some(display)));
                        }
                        buffer.clear();
                    } else {
                        buffer.push(b[0]);
                    }
                }

                let status = child.wait();
                let _ = tx.send(Action::UpdateNxvProgress(None));
                
                if let Ok(status) = status {
                    if status.success() {
                        let _ = tx.send(Action::Log(crate::components::command_log::LogEntry::Info("NXV index updated successfully".to_string())));
                    } else {
                        let _ = tx.send(Action::Log(crate::components::command_log::LogEntry::Info(format!("NXV update failed with status: {}", status))));
                    }
                }
            });
        }

        app
    }

    pub fn process_background_results(&mut self) {
        while let Ok(action) = self.rx.try_recv() {
            self.update(action);
        }
    }

    pub fn fetch_package_details(&mut self) {
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

    pub fn perform_package_search(&mut self) {
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

                let a_exact = a_name_lower == query_lower;
                let b_exact = b_name_lower == query_lower;
                if a_exact != b_exact {
                    return if a_exact {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                let a_starts = a_name_lower.starts_with(&query_lower);
                let b_starts = b_name_lower.starts_with(&query_lower);
                if a_starts != b_starts {
                    return if a_starts {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                let a_contains = a_name_lower.contains(&query_lower);
                let b_contains = b_name_lower.contains(&query_lower);
                if a_contains != b_contains {
                    return if a_contains {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

                let a_desc_contains = a.description.to_lowercase().contains(&query_lower);
                let b_desc_contains = b.description.to_lowercase().contains(&query_lower);
                if a_desc_contains != b_desc_contains {
                    return if a_desc_contains {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }

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

    pub fn start_fetching_suggestions(&mut self) {
        self.domain.suggestions.is_loading = true;
        Suggestions::fetch_branches(self.tx.clone());
    }

    pub fn update_suggestions(&mut self) {
        let existing_urls: Vec<String> = self.domain.inputs.iter().map(|i| i.url.clone()).collect();
        self.domain
            .suggestions
            .update_filtered(&self.ui.new_input_name, &existing_urls);
    }

    pub fn apply_template_logic(&mut self, template_name: String) {
        if let Some(config_dir) = dirs::config_dir() {
            let template_path = config_dir.join("nui").join("templates").join(&template_name);
            crate::log_output("Debug", format!("Applying template: {} (Mode: {:?})", template_name, self.mode));
            
            if template_path.exists() {
                if self.mode == Mode::Shell {
                    match std::fs::read_to_string(&template_path) {
                        Ok(content) => {
                            let pkgs = extract_packages_from_template(&content);
                            crate::log_output("Debug", format!("Extracted {} packages from template: {:?}", pkgs.len(), pkgs));
                            
                            for pkg in pkgs {
                                let attribute = pkg.clone();
                                let pkg_to_add = if let Some(hash) = self.domain.system_nixpkgs_hash.as_ref() {
                                    format!("system/{}#{}", hash, attribute)
                                } else {
                                    attribute.clone()
                                };

                                if !self.shell_packages.contains(&pkg_to_add) {
                                    crate::log_output("Debug", format!("Adding to shell: {}", pkg_to_add));
                                    self.shell_packages.push(pkg_to_add.clone());
                                    self.fetch_shell_package_metadata(pkg_to_add);
                                }
                            }
                            crate::log_output("Success", format!("Added packages from template: {}", template_name));
                            self.ui.show_templates = false;
                        }
                        Err(e) => {
                            crate::log_output("Error", format!("Failed to read template: {}", e));
                        }
                    }
                } else {
                    match std::fs::copy(&template_path, "flake.nix") {
                        Ok(_) => {
                            crate::log_output("Success", format!("Applied template: {}", template_name));
                            self.ui.show_templates = false;
                            self.update(Action::RefreshContext);
                        }
                        Err(e) => {
                            crate::log_output("Error", format!("Failed to apply template: {}", e));
                        }
                    }
                }
            } else {
                crate::log_output("Error", format!("Template not found: {:?}", template_path));
            }
        }
    }

    pub fn fetch_shell_package_metadata(&mut self, pkg_id: String) {
        let tx = self.tx.clone();
        
        // Extract the clean attribute name for nix-env -qa
        let pkg_name = if pkg_id.contains('#') {
            pkg_id.split('#').last().unwrap_or(&pkg_id)
                  .split('@').next().unwrap_or(&pkg_id)
                  .to_string()
        } else if pkg_id.contains('@') {
            pkg_id.split('@').next().unwrap_or(&pkg_id).to_string()
        } else {
            pkg_id.clone()
        };

        std::thread::spawn(move || {
            let output = std::process::Command::new("nix-env")
                .args(["-qa", &pkg_name, "--json"])
                .output();
            
            if let Ok(output) = output {
                if output.status.success() {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        if let Some(obj) = json.as_object() {
                            if let Some(meta) = obj.values().next() {
                                let version = meta.get("version").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                                let description = meta.get("meta").and_then(|m| m.get("description")).and_then(|d| d.as_str()).unwrap_or("").to_string();
                                let _ = tx.send(Action::AddPackageInfo(pkg_id, (description, version, false, String::new())));
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn update(&mut self, action: Action) {
        let context = crate::context::Context::new(self.tx.clone());
        crate::handlers::handle_action(self, &context, action);
    }
}
