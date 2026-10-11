use super::process::{ChildRegistry, stdout};
use super::search::SearchPackage;
use super::{Error, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};

/// Older nxv releases emitted list fields as stringified JSON arrays.
fn string_or_seq<'de, D>(deserializer: D) -> std::result::Result<Option<Vec<String>>, D::Error>
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
struct NxvPackage {
    attribute_path: String,
    version: String,
    description: Option<String>,
    #[serde(default, deserialize_with = "string_or_seq")]
    platforms: Option<Vec<String>>,
    #[serde(default, deserialize_with = "string_or_seq")]
    license: Option<Vec<String>>,
    last_commit_hash: String,
}

#[derive(Debug, Deserialize)]
struct NxvVersion {
    version: String,
    last_commit_hash: String,
    last_commit_date: String,
    #[serde(default, deserialize_with = "string_or_seq")]
    license: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
    pub version: String,
    pub hash: String,
    pub date: String,
    pub is_unfree: bool,
    pub is_system: bool,
}

pub fn is_unfree<S: AsRef<str>>(licenses: &[S]) -> bool {
    licenses
        .iter()
        .any(|l| l.as_ref().to_lowercase().contains("unfree"))
}

pub fn search(query: &str, reg: Option<&ChildRegistry>) -> Result<Vec<SearchPackage>> {
    let mut cmd = Command::new("nxv");
    cmd.args(["search", "-f", "json", "--sort", "date", query]);
    parse_search(&stdout("nxv", cmd, reg)?)
}

/// Parses `nxv search -f json` output, keeping the first (newest) entry per attribute.
pub fn parse_search(json: &[u8]) -> Result<Vec<SearchPackage>> {
    let packages: Vec<NxvPackage> =
        serde_json::from_slice(json).map_err(|e| Error::Parse("nxv", e))?;

    let mut seen = HashSet::new();
    Ok(packages
        .into_iter()
        .filter(|p| seen.insert(p.attribute_path.clone()))
        .map(|p| SearchPackage {
            attribute: p.attribute_path,
            version: Some(p.version),
            description: p.description,
            platforms: p.platforms,
            licenses: p.license,
            hash: Some(p.last_commit_hash),
        })
        .collect())
}

/// Every nixpkgs revision that shipped a distinct version of `attr`.
pub fn versions(attr: &str) -> Result<Vec<VersionInfo>> {
    let mut cmd = Command::new("nxv");
    cmd.args(["search", "-e", attr, "--format", "json"]);
    parse_versions(&stdout("nxv", cmd, None)?)
}

/// Parses `nxv search -e <attr> --format json` output into version history entries.
pub fn parse_versions(json: &[u8]) -> Result<Vec<VersionInfo>> {
    let versions: Vec<NxvVersion> =
        serde_json::from_slice(json).map_err(|e| Error::Parse("nxv", e))?;

    Ok(versions
        .into_iter()
        .map(|v| VersionInfo {
            is_unfree: v.license.as_deref().is_some_and(is_unfree),
            date: v
                .last_commit_date
                .split('T')
                .next()
                .unwrap_or("")
                .to_string(),
            version: v.version,
            hash: v.last_commit_hash,
            is_system: false,
        })
        .collect())
}

/// Runs `nxv update`, reporting each progress line as a compact bar.
pub fn update(mut progress: impl FnMut(String)) -> Result<ExitStatus> {
    // `script` gives nxv a pty so it prints its progress bar.
    let mut child = Command::new("script")
        .args(["-q", "-c", "nxv update", "/dev/null"])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Spawn("nxv update", e))?;

    let mut reader = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while reader.read_exact(&mut byte).is_ok() {
        if byte[0] != b'\n' && byte[0] != b'\r' {
            line.push(byte[0]);
            continue;
        }
        let text = strip_ansi(&String::from_utf8_lossy(&line));
        let text = text.trim();
        if !text.is_empty() {
            progress(compact_progress(text));
        }
        line.clear();
    }
    child.wait().map_err(|e| Error::Spawn("nxv update", e))
}

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            result.push(c);
        } else if chars.next() == Some('[') {
            for c in chars.by_ref() {
                if (0x40..=0x7e).contains(&(c as u32)) {
                    break;
                }
            }
        }
    }
    result
}

pub fn compact_progress(line: &str) -> String {
    let Some(start) = line.find('[') else {
        return line.to_string();
    };
    let Some(end) = line.find(']') else {
        return line[start..].to_string();
    };
    let bar = &line[start + 1..end];
    let total = bar.chars().count();
    let filled = bar
        .chars()
        .filter(|&c| !c.is_whitespace() && c != '-')
        .count();
    let ratio = if total > 0 {
        filled as f32 / total as f32
    } else {
        0.0
    };
    let segments = (ratio * 10.0).round() as usize;
    format!(
        "[{}{}] {}",
        "█".repeat(segments),
        " ".repeat(10 - segments),
        line[end + 1..].trim()
    )
}
