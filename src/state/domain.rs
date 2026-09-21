use crate::components::command_log::LogEntry;
use crate::context::NixFile;
use crate::nix::{Input, Output, suggestions::Suggestions};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// One spawned child process, shared between the thread waiting on it and whoever may
/// cancel it. `None` once the child has been taken for reaping.
pub type ChildSlot = Arc<Mutex<Option<Child>>>;

/// All child processes belonging to the current search generation.
pub type ChildRegistry = Arc<Mutex<Vec<ChildSlot>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Kill and reap every child registered so far, then empty the registry.
pub fn cancel_registered(reg: &ChildRegistry) {
    let slots: Vec<ChildSlot> = std::mem::take(&mut *lock(reg));
    for slot in slots {
        let mut guard = lock(&slot);
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Run `cmd` capturing stdout, registering the child so a newer search can kill it.
///
/// stderr is discarded rather than captured: every caller ignores it, and it must never be
/// inherited or it would corrupt the TUI. Discarding it also removes the pipe-fill deadlock
/// that reading two pipes sequentially on one thread would risk.
fn run_registered(
    mut cmd: Command,
    reg: Option<&ChildRegistry>,
) -> std::io::Result<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn()?;

    let Some(reg) = reg else {
        return child.wait_with_output();
    };

    let mut pipe = child.stdout.take();
    let slot: ChildSlot = Arc::new(Mutex::new(Some(child)));
    lock(reg).push(Arc::clone(&slot));

    let mut stdout = Vec::new();
    if let Some(p) = pipe.as_mut() {
        let _ = p.read_to_end(&mut stdout);
    }
    drop(pipe);

    let mut guard = lock(&slot);
    let status = match guard.take() {
        Some(mut child) => child.wait()?,
        // Already reaped by cancel_registered.
        None => return Err(std::io::Error::other("search cancelled")),
    };

    Ok(std::process::Output {
        status,
        stdout,
        stderr: Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SearchPackage {
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

#[derive(Debug, Deserialize)]
pub struct NixSearchCliPackage {
    pub package_attr_name: String,
    pub package_pname: Option<String>,
    pub package_pversion: Option<String>,
    pub package_description: Option<String>,
    pub package_platforms: Option<Vec<String>>,
    pub package_license: Option<Vec<NixSearchCliLicense>>,
}

#[derive(Debug, Deserialize)]
pub struct NixSearchCliLicense {
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[allow(dead_code)]
    pub url: Option<String>,
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

pub fn nix_search_cli(
    query: String,
    channel: String,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<SearchPackage>, String> {
    let mapped_channel = if channel.contains("unstable") {
        "unstable".to_string()
    } else {
        // Extract version numbers like 24.11 or 25.11
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
                if !found_dot && version.len() >= 2 {
                    // Could be the start of a version, continue
                } else {
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
    };

    let mut cmd = Command::new("nix-search");
    cmd.args(["--json", "--channel", &mapped_channel, &query]);
    let output = run_registered(cmd, reg)
        .map_err(|e| format!("Failed to execute nix-search: {}. Make sure it's installed and you have an internet connection.", e))?;

    if !output.status.success() {
        return Err(format!(
            "nix-search failed with status: {}. This might be due to a connection issue or an invalid channel.",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let pkg: NixSearchCliPackage = serde_json::from_str(line).ok()?;
            Some(SearchPackage {
                attribute: pkg.package_attr_name,
                pname: pkg.package_pname,
                version: pkg.package_pversion,
                description: pkg.package_description,
                platforms: pkg.package_platforms,
                license_set: pkg
                    .package_license
                    .map(|ls| ls.into_iter().map(|l| l.full_name).collect()),
                hash: None,
            })
        })
        .collect();

    Ok(results)
}

#[derive(Debug, Deserialize)]
struct NixSearchPackage {
    description: Option<String>,
    pname: Option<String>,
    version: Option<String>,
}

#[allow(dead_code)]
pub fn nix_search(query: String, rev: String) -> Result<Vec<SearchPackage>, String> {
    let flake_url = format!("github:NixOS/nixpkgs/{}", rev);
    nix_search_flake(flake_url, query, None)
}

pub fn nix_search_flake(
    flake_url: String,
    query: String,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<SearchPackage>, String> {
    let mut cmd = Command::new("nix");
    cmd.args(["search", "--json", &flake_url, &query]);
    let output = run_registered(cmd, reg)
        .map_err(|e| format!("Failed to execute nix search: {}. Make sure nix is installed and you have an internet connection.", e))?;

    if !output.status.success() {
        return Err(format!(
            "nix search failed with status: {}. If searching a remote flake, check your internet connection.",
            output.status
        ));
    }

    let results: HashMap<String, NixSearchPackage> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nix search output: {}", e))?;

    Ok(results
        .into_iter()
        .map(|(attr, pkg)| SearchPackage {
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

fn string_or_seq<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrSeq {
        Seq(Vec<String>),
        Str(String),
    }

    Ok(match Option::<StringOrSeq>::deserialize(deserializer)? {
        Some(StringOrSeq::Seq(v)) => Some(v),
        Some(StringOrSeq::Str(s)) => serde_json::from_str::<Vec<String>>(&s).ok(),
        None => None,
    })
}

#[derive(Debug, Deserialize)]
pub struct NXVPackage {
    pub name: String,
    pub version: String,
    pub attribute_path: String,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "string_or_seq")]
    pub platforms: Option<Vec<String>>,
    #[serde(default, deserialize_with = "string_or_seq")]
    pub license: Option<Vec<String>>,
    pub last_commit_hash: String,
    #[allow(dead_code)]
    pub last_commit_date: String,
}

pub fn nxv_search(
    query: String,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<SearchPackage>, String> {
    let mut cmd = Command::new("nxv");
    cmd.args(["search", "-f", "json", "--sort", "date", &query]);
    let output = run_registered(cmd, reg)
        .map_err(|e| format!("Failed to execute nxv: {}. Make sure nxv is installed and you have an internet connection.", e))?;

    if !output.status.success() {
        return Err(format!(
            "nxv search failed with status: {}. This might be due to a connection issue.",
            output.status
        ));
    }

    let results: Vec<NXVPackage> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nxv output: {}", e))?;

    let mut seen = HashSet::new();
    let filtered = results
        .into_iter()
        .filter(|p| seen.insert(p.attribute_path.clone()))
        .map(|p| {
            SearchPackage {
                attribute: p.attribute_path,
                pname: Some(p.name),
                version: Some(p.version),
                description: p.description,
                platforms: p.platforms,
                license_set: p.license,
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

pub fn fetch_system_versions_batch(
    attrs: Vec<String>,
    reg: Option<&ChildRegistry>,
) -> HashMap<String, String> {
    if attrs.is_empty() {
        return HashMap::new();
    }

    let mut expr = "let pkgs = import <nixpkgs> {}; \
                    lib = pkgs.lib; \
                    getV = pathStr: let \
                      path = lib.splitString \".\" pathStr; \
                      pkg = lib.attrByPath path null pkgs; \
                      res = if pkg != null then builtins.tryEval (pkg.version or \"Unknown\") else { success = false; }; \
                    in if res.success then res.value else \"Unknown\"; \
                    in { ".to_string();

    for attr in &attrs {
        // Simple sanitization: only allow alphanumeric, dots, underscores, hyphens
        if attr.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            expr.push_str(&format!("\"{}\" = getV \"{}\"; ", attr, attr));
        }
    }
    expr.push('}');

    let mut cmd = Command::new("nix");
    cmd.args(["eval", "--json", "--impure", "--expr", &expr]);
    let output = run_registered(cmd, reg);

    if let Ok(output) = output {
        if output.status.success() {
            return serde_json::from_slice(&output.stdout).unwrap_or_default();
        }
    }

    HashMap::new()
}

pub fn fetch_accurate_version(
    rev: String,
    attribute: String,
    reg: Option<&ChildRegistry>,
) -> Option<String> {
    let flake_url = format!("github:NixOS/nixpkgs/{}#{}", rev, attribute);
    let mut cmd = Command::new("nix");
    cmd.args(["eval", "--json", &format!("{}.version", flake_url)]);
    let output = run_registered(cmd, reg).ok()?;

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
    #[serde(default)]
    pub is_system: bool,
}

#[derive(Debug, Deserialize)]
struct NXVResult {
    version: String,
    last_commit_hash: String,
    last_commit_date: String,
    #[serde(default, deserialize_with = "string_or_seq")]
    license: Option<Vec<String>>,
}

pub fn fetch_package_versions(pkg: &str) -> Result<Vec<VersionInfo>, String> {
    let output = std::process::Command::new("nxv")
        .args(["search", "-e", pkg, "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to execute nxv: {}. Make sure nxv is installed and you have an internet connection.", e))?;

    if !output.status.success() {
        return Err(format!(
            "nxv search failed with status: {}. This might be due to a connection issue.",
            output.status
        ));
    }

    let results: Vec<NXVResult> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse nxv output: {}", e))?;

    Ok(results
        .into_iter()
        .map(|r| {
            let is_unfree = r
                .license
                .as_ref()
                .map(|ls| ls.iter().any(|l| l.to_lowercase().contains("unfree")))
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
                is_system: false,
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

pub fn get_available_inputs(
    inputs: &[Input],
    package_name: &str,
    search_results: &[SearchResult],
) -> Vec<(Input, String)> {
    let result = match search_results.iter().find(|res| res.name == package_name) {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut available = Vec::new();
    for input in inputs {
        let channel_name = extract_channel(input);
        if let Some(cv) = result.versions.iter().find(|v| v.channel == channel_name) {
            let version = cv.locked_version.as_ref().unwrap_or(&cv.version).clone();
            available.push((input.clone(), version));
        }
    }
    available
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
    pub nix_search_cli_version: Option<String>,
    pub nxv_version: Option<String>,
    pub system_nixpkgs_version: Option<String>,
    pub system_nixpkgs_hash: Option<String>,
    pub system_nixpkgs_path: Option<String>,
    pub nxv_update_progress: Option<String>,
}

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use std::time::Instant;

    /// A registered child must be killed and reaped when a newer search cancels it, and the
    /// waiting caller must return promptly instead of blocking for the child's full runtime.
    #[test]
    fn cancel_registered_kills_in_flight_child() {
        let reg = ChildRegistry::default();
        let reg_worker = Arc::clone(&reg);

        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            let mut cmd = Command::new("sleep");
            cmd.arg("30");
            run_registered(cmd, Some(&reg_worker))
        });

        // Wait until the child is actually registered, then cancel it.
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while lock(&reg).is_empty() {
            assert!(Instant::now() < deadline, "child was never registered");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cancel_registered(&reg);

        let result = worker.join().expect("worker panicked");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "cancelled child was not killed promptly"
        );
        match result {
            Ok(output) => assert!(!output.status.success(), "killed child reported success"),
            Err(e) => assert_eq!(e.to_string(), "search cancelled"),
        }
        assert!(lock(&reg).is_empty(), "registry was not drained");
    }

    /// Without a registry the runner still behaves like `Command::output()`.
    #[test]
    fn run_registered_without_registry_captures_stdout() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let out = run_registered(cmd, None).expect("echo failed to run");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NXV_SAMPLE: &str = r#"[
      {
        "id": 1769790,
        "name": "ripgrep",
        "version": "15.2.0",
        "first_commit_hash": "d4a57f16",
        "first_commit_date": "2026-07-16T00:20:00Z",
        "last_commit_hash": "c8007378",
        "last_commit_date": "2026-09-21T09:37:33Z",
        "attribute_path": "ripgrep",
        "description": "Fast grep",
        "license": ["Unlicense", "MIT"],
        "homepage": "https://github.com/BurntSushi/ripgrep",
        "maintainers": ["globin"],
        "platforms": ["x86_64-linux", "aarch64-darwin"],
        "source_path": "pkgs/by-name/ri/ripgrep/package.nix",
        "known_vulnerabilities": null
      }
    ]"#;

    // Older nxv releases emitted these fields as stringified JSON arrays.
    const NXV_LEGACY_SAMPLE: &str = r#"[
      {
        "name": "steam",
        "version": "1.0",
        "attribute_path": "steam",
        "description": null,
        "license": "[\"unfreeRedistributable\"]",
        "platforms": "[\"x86_64-linux\"]",
        "last_commit_hash": "abc",
        "last_commit_date": "2026-09-21T09:37:33Z"
      }
    ]"#;

    #[test]
    fn parses_nxv_package_list() {
        let pkgs: Vec<NXVPackage> = serde_json::from_str(NXV_SAMPLE).expect("parse NXVPackage");
        assert_eq!(pkgs[0].attribute_path, "ripgrep");
        assert_eq!(
            pkgs[0].license.as_deref(),
            Some(&["Unlicense".to_string(), "MIT".to_string()][..])
        );
        assert_eq!(
            pkgs[0].platforms.as_deref(),
            Some(&["x86_64-linux".to_string(), "aarch64-darwin".to_string()][..])
        );
    }

    #[test]
    fn parses_nxv_version_list() {
        let results: Vec<NXVResult> = serde_json::from_str(NXV_SAMPLE).expect("parse NXVResult");
        assert_eq!(results[0].version, "15.2.0");
        assert_eq!(results[0].last_commit_hash, "c8007378");
    }

    #[test]
    fn parses_legacy_stringified_arrays() {
        let pkgs: Vec<NXVPackage> =
            serde_json::from_str(NXV_LEGACY_SAMPLE).expect("parse legacy NXVPackage");
        assert_eq!(pkgs[0].platforms.as_deref(), Some(&["x86_64-linux".to_string()][..]));
        let results: Vec<NXVResult> =
            serde_json::from_str(NXV_LEGACY_SAMPLE).expect("parse legacy NXVResult");
        assert_eq!(
            results[0].license.as_deref(),
            Some(&["unfreeRedistributable".to_string()][..])
        );
    }
}
