use crate::components::command_log::LogEntry;
use crate::context::NixFile;
use crate::nix::{Input, Output, suggestions::Suggestions};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    #[serde(rename = "package_license_set")]
    pub license_set: Option<Vec<String>>,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelVersion {
    pub version: String,
    pub channel: String,
    pub locked_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub versions: Vec<ChannelVersion>,
    pub platforms: Vec<String>,
    pub is_unfree: bool,
    pub source_input: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NHSearchResponse {
    pub results: Vec<NHPackage>,
}

pub fn nh_search(query: String, channel: String) -> Result<Vec<NHPackage>, String> {
    let output = std::process::Command::new("nh")
        .args([
            "search",
            "--json",
            "--platforms",
            "--channel",
            &channel,
            &query,
        ])
        .output()
        .map_err(|e| format!("Failed to execute nh: {}", e))?;

    if !output.status.success() {
        return Err(format!("nh search failed with status: {}", output.status));
    }

    let response: NHSearchResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nh output: {}", e))?;

    Ok(response.results)
}

#[derive(Debug, Deserialize)]
struct NixSearchPackage {
    description: Option<String>,
    pname: Option<String>,
    version: Option<String>,
}

pub fn nix_search(query: String, rev: String) -> Result<Vec<NHPackage>, String> {
    let flake_url = format!("github:NixOS/nixpkgs/{}", rev);
    nix_search_flake(flake_url, query)
}

pub fn nix_search_flake(flake_url: String, query: String) -> Result<Vec<NHPackage>, String> {
    let output = std::process::Command::new("nix")
        .args(["search", "--json", &flake_url, &query])
        .output()
        .map_err(|e| format!("Failed to execute nix search: {}", e))?;

    if !output.status.success() {
        return Err(format!("nix search failed with status: {}", output.status));
    }

    let results: HashMap<String, NixSearchPackage> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nix search output: {}", e))?;

    Ok(results
        .into_iter()
        .map(|(attr, pkg)| NHPackage {
            attribute: attr,
            pname: pkg.pname,
            version: pkg.version,
            description: pkg.description,
            platforms: None,
            license_set: None,
            hash: None,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
pub struct NXVPackage {
    pub name: String,
    pub version: String,
    pub attribute_path: String,
    pub description: Option<String>,
    pub platforms: Option<String>,
    pub license: Option<String>,
    pub last_commit_hash: String,
    #[allow(dead_code)]
    pub last_commit_date: String,
}

pub fn nxv_search(query: String) -> Result<Vec<NHPackage>, String> {
    let output = std::process::Command::new("nxv")
        .args(["search", "-f", "json", "--sort", "date", &query])
        .output()
        .map_err(|e| format!("Failed to execute nxv: {}", e))?;

    if !output.status.success() {
        return Err(format!("nxv search failed with status: {}", output.status));
    }

    let results: Vec<NXVPackage> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nxv output: {}", e))?;

    let mut seen = HashSet::new();
    let filtered = results
        .into_iter()
        .filter(|p| seen.insert(p.attribute_path.clone()))
        .map(|p| {
            let platforms = p
                .platforms
                .and_then(|ps| serde_json::from_str::<Vec<String>>(&ps).ok());
            let license_set = p
                .license
                .and_then(|ls| serde_json::from_str::<Vec<String>>(&ls).ok());

            NHPackage {
                attribute: p.attribute_path,
                pname: Some(p.name),
                version: Some(p.version),
                description: p.description,
                platforms,
                license_set,
                hash: Some(p.last_commit_hash),
            }
        })
        .collect();

    Ok(filtered)
}

pub fn extract_channel(input: &Input) -> String {
    if let Some(branch) = &input.branch {
        return branch.clone();
    }
    if let Some(rev) = &input.rev {
        return rev.clone();
    }
    extract_upstream_channel(input)
}

pub fn extract_upstream_channel(input: &Input) -> String {
    let url = &input.url;
    if url.contains("nixpkgs") {
        for part in url.split('/') {
            if part.starts_with("nixos-") || part.starts_with("nixpkgs-") {
                return part.strip_suffix(".tar.gz").unwrap_or(part).to_string();
            }
        }
    }
    "nixos-unstable".to_string()
}

pub fn fetch_accurate_version(rev: String, attribute: String) -> Option<String> {
    let flake_url = format!("github:NixOS/nixpkgs/{}#{}", rev, attribute);
    let output = std::process::Command::new("nix")
        .args(["eval", "--json", &format!("{}.version", flake_url)])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice::<String>(&output.stdout).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionInfo {
    pub version: String,
    pub hash: String,
    pub date: String,
    pub is_unfree: bool,
}

#[derive(Debug, Deserialize)]
struct NXVResult {
    version: String,
    last_commit_hash: String,
    last_commit_date: String,
    license: Option<String>,
}

pub fn fetch_package_versions(pkg: &str) -> Result<Vec<VersionInfo>, String> {
    let output = std::process::Command::new("nxv")
        .args(["search", "-e", pkg, "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to execute nxv: {}", e))?;

    if !output.status.success() {
        return Err(format!("nxv search failed with status: {}", output.status));
    }

    let results: Vec<NXVResult> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nxv output: {}", e))?;

    Ok(results
        .into_iter()
        .map(|r| {
            let is_unfree = r
                .license
                .as_ref()
                .map(|l| l.to_lowercase().contains("unfree"))
                .unwrap_or(false);
            VersionInfo {
                version: r.version,
                hash: r.last_commit_hash,
                date: r
                    .last_commit_date
                    .split('T')
                    .next()
                    .unwrap_or("")
                    .to_string(),
                is_unfree,
            }
        })
        .collect())
}

pub fn get_tool_version(cmd: &str) -> Option<String> {
    let output = std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .ok()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next()?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();

        if parts.len() >= 2 && parts[0].to_lowercase() == cmd.to_lowercase() {
            Some(parts[1].to_string())
        } else {
            // Fallback to the last word if it doesn't start with the command name
            Some(parts.last()?.to_string())
        }
    } else {
        None
    }
}

#[derive(Default)]
pub struct DomainData {
    pub nix_files: Vec<NixFile>,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub package_info: HashMap<String, (String, String, bool, String)>,
    pub package_search_results: Vec<SearchResult>,
    pub searched_channels: Vec<String>,
    pub suggestions: Suggestions,
    pub logs: Vec<LogEntry>,
    pub pending_fetches: HashSet<String>,
    pub package_versions: Vec<VersionInfo>,
    pub package_updates: HashMap<String, String>,
    pub nh_version: Option<String>,
    pub nxv_version: Option<String>,
}
