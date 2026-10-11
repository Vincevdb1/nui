use super::Result;
use super::process::{ChildRegistry, stdout};
use super::search::SearchPackage;
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize)]
struct CliPackage {
    package_attr_name: String,
    package_pversion: Option<String>,
    package_description: Option<String>,
    package_platforms: Option<Vec<String>>,
    package_license: Option<Vec<CliLicense>>,
}

#[derive(Deserialize)]
struct CliLicense {
    #[serde(rename = "fullName")]
    full_name: String,
}

/// nix-search only knows `unstable` and release numbers such as `25.11`.
pub fn cli_channel(channel: &str) -> String {
    if channel.contains("unstable") {
        return "unstable".to_string();
    }
    let mut version = String::new();
    let mut found_dot = false;
    for c in channel.chars() {
        if c.is_ascii_digit() {
            version.push(c);
        } else if c == '.' && !version.is_empty() && !found_dot {
            version.push(c);
            found_dot = true;
        } else if !version.is_empty() {
            if found_dot && version.len() >= 4 {
                break;
            }
            if found_dot || version.len() < 2 {
                version.clear();
                found_dot = false;
            }
        }
    }
    if version.len() >= 4 && found_dot {
        version
    } else {
        "unstable".to_string()
    }
}

pub fn search(
    query: &str,
    channel: &str,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<SearchPackage>> {
    let mut cmd = Command::new("nix-search");
    cmd.args(["--json", "--channel", &cli_channel(channel), query]);
    Ok(parse_output(&stdout("nix-search", cmd, reg)?))
}

/// Parses `nix-search --json` output, one JSON object per line; unparsable lines are skipped.
pub fn parse_output(out: &[u8]) -> Vec<SearchPackage> {
    String::from_utf8_lossy(out)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<CliPackage>(line).ok())
        .map(|pkg| SearchPackage {
            attribute: pkg.package_attr_name,
            version: pkg.package_pversion,
            description: pkg.package_description,
            platforms: pkg.package_platforms,
            licenses: pkg
                .package_license
                .map(|ls| ls.into_iter().map(|l| l.full_name).collect()),
            hash: None,
        })
        .collect()
}
