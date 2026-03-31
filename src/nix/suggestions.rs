use ratatui::widgets::ListState;
use serde::Deserialize;

#[derive(Deserialize)]
struct GitHubBranch {
    name: String,
}

#[derive(Default)]
pub struct Suggestions {
    pub all: Vec<(String, String)>,
    pub filtered: Vec<(String, String)>,
    pub selected_index: usize,
    pub list_state: ListState,
    pub is_loading: bool,
}

impl Suggestions {
    pub fn update_filtered(&mut self, query: &str, existing_urls: &[String]) {
        let query_lower = query.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .filter(|(name, url)| {
                name.to_lowercase().contains(&query_lower) && !existing_urls.contains(url)
            })
            .cloned()
            .collect();

        if self.selected_index >= self.filtered.len() {
            self.selected_index = 0;
        }
        self.list_state.select(if self.filtered.is_empty() {
            None
        } else {
            Some(self.selected_index)
        });
    }

    pub fn fetch_branches(tx: std::sync::mpsc::Sender<Vec<(String, String)>>) {
        crate::log_action("Fetching nixpkgs branches", "GET https://api.github.com/repos/nixos/nixpkgs/branches");
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("nui-tui-app")
                .build();

            if let Ok(client) = client {
                // Using protected=true usually gives us the release branches we care about
                let response = client.get("https://api.github.com/repos/nixos/nixpkgs/branches?protected=true&per_page=100")
                    .send();

                match response {
                    Ok(response) => {
                        if response.status().is_success() {
                            if let Ok(branches_json) = response.json::<Vec<GitHubBranch>>() {
                                let branches: Vec<(String, String)> = branches_json
                                    .into_iter()
                                    .filter_map(|b| {
                                        let branch = b.name;
                                        // We want nixos-XX.XX branches, master, and nixpkgs-unstable
                                        if branch.starts_with("nixos-")
                                            || branch == "nixpkgs-unstable"
                                            || branch == "master"
                                        {
                                            let url = format!("github:nixos/nixpkgs/{}", branch);
                                            return Some((branch, url));
                                        }
                                        None
                                    })
                                    .collect();

                                let mut sorted_branches = branches;
                                // Sort by name, but keep master and nixpkgs-unstable at the top if possible
                                sorted_branches.sort_by(|a, b| {
                                    if a.0 == "master" || a.0 == "nixpkgs-unstable" {
                                        std::cmp::Ordering::Less
                                    } else if b.0 == "master" || b.0 == "nixpkgs-unstable" {
                                        std::cmp::Ordering::Greater
                                    } else {
                                        b.0.cmp(&a.0) // Reverse version sort
                                    }
                                });

                                crate::log_output("GitHub Output", format!("Successfully fetched {} branches", sorted_branches.len()));
                                let _ = tx.send(sorted_branches);
                            } else {
                                crate::log_output("GitHub Error", "Failed to parse GitHub API response for branches");
                            }
                        } else {
                            crate::log_output("GitHub Error", format!("GitHub API error: {}", response.status()));
                        }
                    }
                    Err(e) => {
                        crate::log_output("GitHub Error", format!("Failed to fetch branches: {}", e));
                    }
                }
            }
        });
    }
}
