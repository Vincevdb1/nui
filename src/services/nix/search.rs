use super::NixService;
use crate::action::Action;
use crate::nix::Input;
use crate::nix::suggestions::Suggestions;
use crate::state::{
    Mode,
    domain::{self, ChannelVersion, SearchResult},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;

impl NixService {
    pub fn fetch_package_details(
        &self,
        flake_path: String,
        output_type: String,
        output_name: String,
        fetch_id: usize,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res =
                crate::nix::Package::fetch_from_output(&flake_path, &output_type, &output_name);
            let _ = tx.send(Action::SetPackageDetails(fetch_id, res));
        });
    }

    pub fn perform_package_search(
        &self,
        search_id: usize,
        query: String,
        mode: Mode,
        inputs: Vec<Input>,
        _system_nixpkgs_hash: Option<String>,
        system_nixpkgs_version: Option<String>,
    ) {
        let tx = self.tx.clone();

        // Kill whatever the previous query is still running before starting this one.
        self.current_search_id.store(search_id, Ordering::SeqCst);
        domain::cancel_registered(&self.search_children);
        let reg = Arc::clone(&self.search_children);
        let current: Arc<AtomicUsize> = Arc::clone(&self.current_search_id);
        let is_current = move || current.load(Ordering::SeqCst) == search_id;

        let mut search_targets: Vec<String> = if mode == Mode::Shell {
            let mut targets = vec!["nxv".to_string()];
            if let Some(version) = &system_nixpkgs_version {
                targets.push(version.clone());
            } else {
                targets.push("nixos-unstable".to_string());
            }
            targets
        } else {
            let mut targets = Vec::new();
            for input in &inputs {
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
        if mode == Mode::Flake {
            for input in &inputs {
                if !input.url.contains("nixpkgs") {
                    non_nixpkgs_inputs.push(input.clone());
                }
            }
        }

        search_targets.sort();
        search_targets.dedup();

        let non_nixpkgs_names: Vec<String> =
            non_nixpkgs_inputs.iter().map(|i| i.name.clone()).collect();

        std::thread::spawn(move || {
            let mut results_map: HashMap<String, SearchResult> = HashMap::new();

            if mode == Mode::Shell {
                let mut nix_search_err = None;
                let mut nxv_search_err = None;

                let mut threads = Vec::new();
                for target in &search_targets {
                    if target == "nxv" {
                        let q = query.clone();
                        let r = Arc::clone(&reg);
                        threads.push(std::thread::spawn(move || {
                            let res = domain::nxv_search(q, Some(&r));
                            ("nxv".to_string(), res)
                        }));
                    } else {
                        let q = query.clone();
                        let t = target.clone();
                        let r = Arc::clone(&reg);
                        threads.push(std::thread::spawn(move || {
                            let res = domain::nix_search_cli(q, t.clone(), Some(&r));
                            (t, res)
                        }));
                    }
                }

                for t in threads {
                    match t.join() {
                        Ok((source, Ok(packages))) => {
                            for p in packages {
                                let entry =
                                    results_map.entry(p.attribute.clone()).or_insert_with(|| {
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
                                    if license_set
                                        .iter()
                                        .any(|l| l.to_lowercase().contains("unfree"))
                                    {
                                        entry.is_unfree = true;
                                    }
                                }

                                let channel_display = if source == "nxv" {
                                    "nxv".to_string()
                                } else if source.contains('.') || source.contains("git") {
                                    // It's a versioned channel or system version, keep it
                                    source.clone()
                                } else {
                                    "nixos-unstable".to_string()
                                };

                                if !entry.versions.iter().any(|v| v.channel == channel_display) {
                                    entry.versions.push(ChannelVersion {
                                        version: p.version.unwrap_or_else(|| "Unknown".to_string()),
                                        channel: channel_display,
                                        locked_version: None,
                                    });
                                }
                            }
                        }
                        Ok((source, Err(e))) => {
                            if is_current() {
                                crate::log_output(
                                    format!("Search Error ({})", source),
                                    e.clone(),
                                );
                            }
                            if source == "nxv" {
                                nxv_search_err = Some(e);
                            } else {
                                nix_search_err = Some(e);
                            }
                        }
                        Err(_) => {
                            crate::log_output("Thread Error", "A search thread panicked");
                        }
                    }
                }

                // In Shell mode, fetch actual versions from the system nixpkgs for accuracy
                let mut final_results: Vec<SearchResult> = results_map.into_values().collect();
                crate::services::nix::search_sort::sort_search_results(&mut final_results, &query);

                // Fetch system versions for the top 50 results
                let attrs_to_fetch: Vec<String> = final_results
                    .iter()
                    .take(50)
                    .map(|r| r.name.clone())
                    .collect();

                let system_versions = domain::fetch_system_versions_batch(attrs_to_fetch, Some(&reg));

                for res in final_results.iter_mut().take(50) {
                    if let Some(sys_version) = system_versions.get(&res.name) {
                        if sys_version != "Unknown" {
                            // Add or update the system version in the versions list
                            if let Some(v) = res.versions.iter_mut().find(|v| v.channel == "system") {
                                v.version = sys_version.clone();
                            } else {
                                res.versions.insert(
                                    0,
                                    ChannelVersion {
                                        version: sys_version.clone(),
                                        channel: "system".to_string(),
                                        locked_version: None,
                                    },
                                );
                            }
                        }
                    }
                }

                if final_results.is_empty() {
                    if let Some(e) = nix_search_err {
                        let _ = tx.send(Action::SetPackageSearchResults(search_id, Err(e)));
                        return;
                    }
                    if let Some(e) = nxv_search_err {
                        let _ = tx.send(Action::SetPackageSearchResults(search_id, Err(e)));
                        return;
                    }
                }
                let _ = tx.send(Action::SetPackageSearchResults(search_id, Ok(final_results)));
                return;
            } else {
                let mut threads = Vec::new();
                for target in search_targets {
                    let q = query.clone();
                    let t = target.clone();
                    let r = Arc::clone(&reg);
                    threads.push(std::thread::spawn(move || {
                        let res = domain::nix_search_cli(q, t.clone(), Some(&r));
                        (t, res)
                    }));
                }

                for input in non_nixpkgs_inputs {
                    let q = query.clone();
                    let input_name = input.name.clone();
                    let input_url = input.url.clone();
                    let r = Arc::clone(&reg);
                    threads.push(std::thread::spawn(move || {
                        let res = domain::nix_search_flake(input_url, q, Some(&r));
                        (input_name, res)
                    }));
                }

                let q = query.clone();
                let r = Arc::clone(&reg);
                threads.push(std::thread::spawn(move || {
                    let res = domain::nix_search_flake(".".to_string(), q, Some(&r));
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
                                        source_input: if is_input {
                                            Some(source.clone())
                                        } else {
                                            None
                                        },
                                        hash: p.hash.clone(),
                                    }
                                });

                                if entry.hash.is_none() {
                                    entry.hash = p.hash.clone();
                                }

                                if let Some(license_set) = p.license_set {
                                    if license_set
                                        .iter()
                                        .any(|l| l.to_lowercase().contains("unfree"))
                                    {
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
                            if is_current() {
                                crate::log_output(
                                    format!("Search Error ({})", source),
                                    e.clone(),
                                );
                            }
                            search_errors.push(e);
                        }
                        Err(_) => {
                            crate::log_output("Thread Error", "A search thread panicked");
                        }
                    }
                }

                if results_map.is_empty() && !search_errors.is_empty() {
                    let _ = tx.send(Action::SetPackageSearchResults(search_id, Err(
                        search_errors[0].clone()
                    )));
                    return;
                }
            }

            let mut final_results: Vec<SearchResult> = results_map.into_values().collect();
            crate::services::nix::search_sort::sort_search_results(&mut final_results, &query);
            let _ = tx.send(Action::SetPackageSearchResults(search_id, Ok(final_results)));
        });
    }

    pub fn check_for_updates(&self, packages: Vec<(String, String)>, inputs: Vec<Input>) {
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            for (pkg_name, source_input) in packages {
                let channel = inputs
                    .iter()
                    .find(|i| i.name == source_input)
                    .map(|i| domain::extract_upstream_channel(i))
                    .unwrap_or_else(|| "nixos-unstable".to_string());

                let tx = tx.clone();
                let pkg = pkg_name.clone();
                std::thread::spawn(move || match domain::nix_search_cli(pkg.clone(), channel, None) {
                    Ok(results) => {
                        let latest = results.iter().find(|p| p.attribute == pkg).or_else(|| {
                            results
                                .iter()
                                .find(|p| p.attribute.ends_with(&format!(".{}", pkg)))
                        });

                        if let Some(latest) = latest {
                            if let Some(version) = &latest.version {
                                let _ = tx.send(Action::UpdatePackageVersion(pkg, version.clone()));
                            }
                        }
                    }
                    Err(e) => {
                        crate::log_output(format!("Update Check Error ({})", pkg), e);
                    }
                });
            }
        });
    }

    pub fn start_fetching_suggestions(&self) {
        Suggestions::fetch_branches(self.tx.clone());
    }

    pub fn fetch_shell_package_metadata(&self, pkg_id: String) {
        let tx = self.tx.clone();

        // Extract the clean attribute name for nix-env -qa
        let pkg_name = if pkg_id.contains('#') {
            pkg_id
                .split('#')
                .last()
                .unwrap_or(&pkg_id)
                .split('@')
                .next()
                .unwrap_or(&pkg_id)
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
                                let version = meta
                                    .get("version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                let description = meta
                                    .get("meta")
                                    .and_then(|m| m.get("description"))
                                    .and_then(|d| d.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let _ = tx.send(Action::AddPackageInfo(
                                    pkg_id,
                                    (description, version, false, String::new()),
                                ));
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn fetch_package_versions(&self, pkg: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = domain::fetch_package_versions(&pkg);
            let _ = tx.send(Action::SetVersions(res));
        });
    }
}
