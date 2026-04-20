use crate::components::command_log::LogEntry;
use crate::context::NixFile;
use crate::nix::{Configuration, Input, suggestions::Suggestions};
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
    pub is_unfree: bool,
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

pub fn extract_channel(url: &str) -> String {
    if url.contains("nixpkgs") {
        for part in url.split('/') {
            if part.starts_with("nixos-") || part.starts_with("nixpkgs-") {
                return part.strip_suffix(".tar.gz").unwrap_or(part).to_string();
            }
        }
    }
    "nixos-unstable".to_string()
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
            let is_unfree = r.license.as_ref().map(|l| l.to_lowercase().contains("unfree")).unwrap_or(false);
            VersionInfo {
                version: r.version,
                hash: r.last_commit_hash,
                date: r.last_commit_date.split('T').next().unwrap_or("").to_string(),
                is_unfree,
            }
        })
        .collect())
}

#[derive(Default)]
pub struct DomainData {
    pub nix_files: Vec<NixFile>,
    pub inputs: Vec<Input>,
    pub configurations: Vec<Configuration>,
    pub package_info: HashMap<String, (String, String, bool)>,
    pub package_search_results: Vec<SearchResult>,
    pub searched_channels: Vec<String>,
    pub suggestions: Suggestions,
    pub logs: Vec<LogEntry>,
    pub pending_fetches: HashSet<String>,
    pub package_versions: Vec<VersionInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nxv_json() {
        let json = r#"[
  {
    "id": 1247933,
    "name": "ripgrep",
    "version": "15.1.0",
    "first_commit_hash": "ac9dd6865391c38a62c08f491467455ce51ff34e",
    "first_commit_date": "2026-02-07T09:19:40Z",
    "last_commit_hash": "605ce345a1573958ede124887c77fed02eb6b860",
    "last_commit_date": "2026-02-07T12:02:24Z",
    "attribute_path": "ripgrep",
    "description": "Utility that combines the usability of The Silver Searcher with the raw speed of grep",
    "license": "[\"MIT\",\"Unlicense\"]",
    "homepage": "https://github.com/BurntSushi/ripgrep",
    "maintainers": "[\"Ma27\",\"globin\",\"zowoq\"]",
    "platforms": "[\"aarch64-darwin\"]",
    "source_path": "pkgs/by-name/ri/ripgrep/package.nix",
    "known_vulnerabilities": null
  }
]"#;
        let results: Vec<NXVResult> = serde_json::from_str(json).unwrap();
        let versions: Vec<VersionInfo> = results
            .into_iter()
            .map(|r| VersionInfo {
                version: r.version,
                hash: r.last_commit_hash,
                date: r.last_commit_date.split('T').next().unwrap_or("").to_string(),
            })
            .collect();

        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "15.1.0");
        assert_eq!(versions[0].hash, "605ce345a1573958ede124887c77fed02eb6b860");
        assert_eq!(versions[0].date, "2026-02-07");
    }
}
