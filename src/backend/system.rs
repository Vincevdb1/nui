use super::process;
use std::process::Command;

/// The nixpkgs the user's `<nixpkgs>` and NixOS system resolve to.
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub nixpkgs_version: Option<String>,
    pub nixpkgs_path: Option<String>,
    pub nixpkgs_rev: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub nix_search: Option<String>,
    pub nxv: Option<String>,
}

fn trimmed_stdout(cmd: &mut Command) -> Option<String> {
    let output = process::output(cmd).ok().filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// One eval for both values; `lib` alone avoids instantiating all of nixpkgs.
const PROBE_EXPR: &str = "{ version = (import <nixpkgs/lib>).version; path = toString <nixpkgs>; }";

#[derive(serde::Deserialize)]
struct Probe {
    version: String,
    path: String,
}

impl SystemInfo {
    pub fn probe() -> Self {
        let probe = process::output(
            Command::new("nix").args(["eval", "--json", "--impure", "--expr", PROBE_EXPR]),
        )
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout)
        .unwrap_or_default();
        let nixos_rev = trimmed_stdout(Command::new("nixos-version").arg("--revision"));
        Self::from_probe(&probe, nixos_rev)
    }

    /// Builds the info from the probe eval's JSON and `nixos-version --revision`; without a
    /// NixOS revision, the rev is read from the store path's name.
    pub fn from_probe(json: &[u8], nixos_rev: Option<String>) -> Self {
        let probe: Option<Probe> = serde_json::from_slice(json).ok();
        let (nixpkgs_version, nixpkgs_path) = match probe {
            Some(p) => (Some(p.version), Some(p.path).filter(|p| !p.is_empty())),
            None => (None, None),
        };
        let nixpkgs_rev = nixos_rev.filter(|rev| rev.len() >= 7).or_else(|| {
            let name = std::path::Path::new(nixpkgs_path.as_deref()?).file_name()?;
            let rev: String = name.to_string_lossy().chars().take(32).collect();
            (!rev.is_empty()).then_some(rev)
        });
        Self {
            nixpkgs_version,
            nixpkgs_path,
            nixpkgs_rev,
        }
    }
}

impl Tools {
    pub fn probe() -> Self {
        Self {
            nix_search: tool_version("nix-search"),
            nxv: tool_version("nxv"),
        }
    }
}

fn tool_version(cmd: &str) -> Option<String> {
    parse_tool_version(cmd, &trimmed_stdout(Command::new(cmd).arg("--version"))?)
}

/// The version in `<cmd> --version` output: the word after the tool's name, or the last word.
pub fn parse_tool_version(cmd: &str, output: &str) -> Option<String> {
    let first_line = output.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0].eq_ignore_ascii_case(cmd) {
        Some(parts[1].to_string())
    } else {
        parts.last().map(|s| s.to_string())
    }
}
