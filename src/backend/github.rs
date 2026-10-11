use serde::Deserialize;

#[derive(Deserialize)]
struct GitHubBranch {
    name: String,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub url: String,
}

const BRANCHES_URL: &str =
    "https://api.github.com/repos/nixos/nixpkgs/branches?protected=true&per_page=100";

/// Protected nixpkgs branches worth tracking as inputs, rolling branches first.
pub fn nixpkgs_branches() -> Result<Vec<Branch>, String> {
    crate::log::action(
        "Fetching nixpkgs branches",
        "GET https://api.github.com/repos/nixos/nixpkgs/branches",
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("nui-tui-app")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(BRANCHES_URL).send().map_err(|e| {
        format!(
            "Failed to fetch branches: {}. Check your internet connection.",
            e
        )
    })?;
    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }
    let branches: Vec<GitHubBranch> = response
        .json()
        .map_err(|_| "Failed to parse GitHub API response for branches".to_string())?;
    let branches = rank_branches(branches.into_iter().map(|b| b.name).collect());

    crate::log::output(
        "GitHub Output",
        format!("Successfully fetched {} branches", branches.len()),
    );
    Ok(branches)
}

/// Keeps the release and rolling branches, rolling first, then newest release first.
pub fn rank_branches(names: Vec<String>) -> Vec<Branch> {
    let is_rolling = |name: &str| name == "master" || name == "nixpkgs-unstable";
    let mut branches: Vec<Branch> = names
        .into_iter()
        .filter(|name| name.starts_with("nixos-") || is_rolling(name))
        .map(|name| Branch {
            url: format!("github:nixos/nixpkgs/{}", name),
            name,
        })
        .collect();
    branches.sort_by(|a, b| {
        is_rolling(&b.name)
            .cmp(&is_rolling(&a.name))
            .then_with(|| b.name.cmp(&a.name))
    });
    branches
}
