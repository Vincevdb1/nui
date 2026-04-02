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

#[derive(Default)]
pub struct DomainData {
    pub nix_files: Vec<NixFile>,
    pub inputs: Vec<Input>,
    pub configurations: Vec<Configuration>,
    pub package_info: HashMap<String, (String, String)>,
    pub package_search_results: Vec<SearchResult>,
    pub searched_channels: Vec<String>,
    pub suggestions: Suggestions,
    pub logs: Vec<LogEntry>,
    pub pending_fetches: HashSet<String>,
}
