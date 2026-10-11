use super::process::ChildRegistry;
use super::{nix, nix_search, nxv};
use crate::flake::Input;
use std::collections::HashMap;
use std::thread::JoinHandle;

/// A package as reported by any of the search tools.
pub struct SearchPackage {
    pub attribute: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub platforms: Option<Vec<String>>,
    pub licenses: Option<Vec<String>>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChannelVersion {
    pub version: String,
    pub channel: String,
    pub locked_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub versions: Vec<ChannelVersion>,
    pub platforms: Vec<String>,
    pub is_unfree: bool,
    pub source_input: Option<String>,
    pub hash: Option<String>,
}

impl SearchResult {
    fn new(name: String, source_input: Option<String>, hash: Option<String>) -> Self {
        Self {
            name,
            description: String::new(),
            versions: Vec::new(),
            platforms: Vec::new(),
            is_unfree: false,
            source_input,
            hash,
        }
    }

    fn absorb_common(&mut self, pkg: &SearchPackage) {
        if self.hash.is_none() {
            self.hash = pkg.hash.clone();
        }
        if pkg.licenses.as_deref().is_some_and(nxv::is_unfree) {
            self.is_unfree = true;
        }
    }

    /// The version the shell would get: system nixpkgs first, then the system channel.
    pub fn shell_version(&self, system_version: Option<&str>) -> String {
        self.versions
            .iter()
            .find(|v| v.channel == "system")
            .or_else(|| {
                let sys = system_version?;
                self.versions
                    .iter()
                    .find(|v| v.channel == sys || sys.starts_with(&v.channel))
            })
            .or_else(|| self.versions.first())
            .map(|v| v.version.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

pub enum Scope {
    /// nxv history plus the channel matching the system nixpkgs.
    Shell { system_channel: String },
    /// The flake's nixpkgs channels, its other inputs and the flake itself.
    Flake {
        channels: Vec<String>,
        inputs: Vec<Input>,
    },
}

pub struct Request {
    pub query: String,
    pub scope: Scope,
}

impl Request {
    pub fn new(
        query: String,
        shell_mode: bool,
        inputs: &[Input],
        system_version: Option<&str>,
    ) -> Self {
        let scope = if shell_mode {
            Scope::Shell {
                system_channel: system_version.unwrap_or("nixos-unstable").to_string(),
            }
        } else {
            let mut channels: Vec<String> = inputs
                .iter()
                .filter(|i| i.is_nixpkgs())
                .map(Input::upstream_channel)
                .collect();
            if channels.is_empty() {
                channels.push("nixos-unstable".to_string());
            }
            channels.sort();
            channels.dedup();
            Scope::Flake {
                channels,
                inputs: inputs.iter().filter(|i| !i.is_nixpkgs()).cloned().collect(),
            }
        };
        Self { query, scope }
    }

    /// Channels shown as version columns.
    pub fn channels(&self) -> Vec<String> {
        match &self.scope {
            Scope::Shell { system_channel } => {
                let mut channels = vec!["nxv".to_string(), system_channel.clone()];
                channels.sort();
                channels.dedup();
                channels
            }
            Scope::Flake { channels, .. } => channels.clone(),
        }
    }
}

type Merge<'a> = dyn FnMut(&str, SearchPackage, &mut HashMap<String, SearchResult>) + 'a;
type Job = JoinHandle<(String, super::Result<Vec<SearchPackage>>)>;

fn spawn(
    source: String,
    f: impl FnOnce() -> super::Result<Vec<SearchPackage>> + Send + 'static,
) -> Job {
    std::thread::spawn(move || (source, f()))
}

/// Runs every search the scope needs in parallel and merges the results per attribute.
pub fn run(
    request: &Request,
    reg: &ChildRegistry,
    is_current: impl Fn() -> bool,
) -> Result<Vec<SearchResult>, String> {
    let query = &request.query;
    let mut results: HashMap<String, SearchResult> = HashMap::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    let mut collect = |jobs: Vec<Job>, merge: &mut Merge| {
        for job in jobs {
            match job.join() {
                Ok((source, Ok(packages))) => {
                    for pkg in packages {
                        merge(&source, pkg, &mut results);
                    }
                }
                Ok((source, Err(e))) => {
                    if is_current() {
                        crate::log::output(format!("Search Error ({})", source), e.to_string());
                    }
                    errors.push((source, e.to_string()));
                }
                Err(_) => crate::log::output("Thread Error", "A search thread panicked"),
            }
        }
    };

    match &request.scope {
        Scope::Shell { .. } => {
            let jobs = request
                .channels()
                .into_iter()
                .map(|target| {
                    let (q, r) = (query.clone(), reg.clone());
                    if target == "nxv" {
                        spawn(target, move || nxv::search(&q, Some(&r)))
                    } else {
                        let t = target.clone();
                        spawn(target, move || nix_search::search(&q, &t, Some(&r)))
                    }
                })
                .collect();
            collect(jobs, &mut |source, pkg, results| {
                merge_shell(results, source, pkg)
            });
            let mut sorted = sorted(results, query);
            add_system_versions(&mut sorted, reg);
            if sorted.is_empty() {
                let first_error = errors
                    .iter()
                    .find(|(source, _)| source != "nxv")
                    .or_else(|| errors.first());
                if let Some((_, e)) = first_error {
                    return Err(e.clone());
                }
            }
            Ok(sorted)
        }
        Scope::Flake { channels, inputs } => {
            let mut jobs: Vec<Job> = channels
                .iter()
                .map(|channel| {
                    let (q, r, c) = (query.clone(), reg.clone(), channel.clone());
                    spawn(channel.clone(), move || {
                        nix_search::search(&q, &c, Some(&r))
                    })
                })
                .collect();
            for input in inputs {
                let (q, r, url) = (query.clone(), reg.clone(), input.url.clone());
                jobs.push(spawn(input.name.clone(), move || {
                    nix::search_flake(&url, &q, Some(&r))
                }));
            }
            let (q, r) = (query.clone(), reg.clone());
            jobs.push(spawn("self".to_string(), move || {
                nix::search_flake(".", &q, Some(&r))
            }));

            let input_names: Vec<&str> = inputs.iter().map(|i| i.name.as_str()).collect();
            collect(jobs, &mut |source, pkg, results| {
                let is_input = source == "self" || input_names.contains(&source);
                merge_flake(results, source, is_input, pkg)
            });

            if results.is_empty()
                && let Some((_, e)) = errors.into_iter().next()
            {
                return Err(e);
            }
            Ok(sorted(results, query))
        }
    }
}

/// Folds a shell-scope search hit into its attribute's result, one version per channel.
pub fn merge_shell(results: &mut HashMap<String, SearchResult>, source: &str, pkg: SearchPackage) {
    let entry = results
        .entry(pkg.attribute.clone())
        .or_insert_with(|| SearchResult {
            description: pkg.description.clone().unwrap_or_default(),
            platforms: pkg.platforms.clone().unwrap_or_default(),
            ..SearchResult::new(pkg.attribute.clone(), None, pkg.hash.clone())
        });
    if entry.description.is_empty()
        && let Some(desc) = &pkg.description
    {
        entry.description = desc.clone();
    }
    if entry.platforms.is_empty()
        && let Some(platforms) = &pkg.platforms
    {
        entry.platforms = platforms.clone();
    }
    entry.absorb_common(&pkg);

    let channel = if source == "nxv" || source.contains('.') || source.contains("git") {
        source.to_string()
    } else {
        "nixos-unstable".to_string()
    };
    if !entry.versions.iter().any(|v| v.channel == channel) {
        entry.versions.push(ChannelVersion {
            version: pkg.version.unwrap_or_else(|| "Unknown".to_string()),
            channel,
            locked_version: None,
        });
    }
}

/// Folds a flake-scope search hit into its result; hits from inputs (and `self`) are keyed
/// as `<input>.<attr>` so they stay separate from nixpkgs packages.
pub fn merge_flake(
    results: &mut HashMap<String, SearchResult>,
    source: &str,
    is_input: bool,
    pkg: SearchPackage,
) {
    let key = if is_input {
        format!("{}.{}", source, pkg.attribute)
    } else {
        pkg.attribute.clone()
    };
    let entry = results.entry(key.clone()).or_insert_with(|| {
        SearchResult::new(key, is_input.then(|| source.to_string()), pkg.hash.clone())
    });
    entry.absorb_common(&pkg);

    let had_unstable = entry
        .versions
        .iter()
        .any(|v| v.channel.contains("unstable"));
    let desc = pkg.description.clone().unwrap_or_default();
    if entry.description.is_empty()
        || (source.contains("unstable") && !had_unstable)
        || desc.len() > entry.description.len()
    {
        entry.description = desc;
    }
    for platform in pkg.platforms.into_iter().flatten() {
        if !entry.platforms.contains(&platform) {
            entry.platforms.push(platform);
        }
    }
    entry.platforms.sort();
    entry.versions.push(ChannelVersion {
        version: pkg.version.unwrap_or_else(|| "Unknown".to_string()),
        channel: source.to_string(),
        locked_version: None,
    });
}

/// Ranks exact matches, then prefix, substring and description matches, then by name.
pub fn sorted(results: HashMap<String, SearchResult>, query: &str) -> Vec<SearchResult> {
    let query = query.to_lowercase();
    let mut results: Vec<SearchResult> = results.into_values().collect();
    results.sort_by_cached_key(|r| {
        let name = r.name.to_lowercase();
        (
            name != query,
            !name.starts_with(&query),
            !name.contains(&query),
            !r.description.to_lowercase().contains(&query),
            name,
        )
    });
    results
}

/// Puts the system nixpkgs version first for the top results, which the shell uses.
fn add_system_versions(results: &mut [SearchResult], reg: &ChildRegistry) {
    let top: Vec<String> = results.iter().take(50).map(|r| r.name.clone()).collect();
    let versions = nix::system_versions(&top, Some(reg));
    for result in results.iter_mut().take(50) {
        let Some(version) = versions.get(&result.name).filter(|v| *v != "Unknown") else {
            continue;
        };
        match result.versions.iter_mut().find(|v| v.channel == "system") {
            Some(v) => v.version = version.clone(),
            None => result.versions.insert(
                0,
                ChannelVersion {
                    version: version.clone(),
                    channel: "system".to_string(),
                    locked_version: None,
                },
            ),
        }
    }
}

/// Versions of the top results at each nixpkgs input's locked revision.
pub fn locked_versions(
    results: &[SearchResult],
    inputs: &[Input],
    reg: &ChildRegistry,
    mut found: impl FnMut(String, String, String),
) {
    for result in results.iter().take(10).filter(|r| r.source_input.is_none()) {
        for input in inputs.iter().filter(|i| i.is_nixpkgs()) {
            if let Some(rev) = &input.rev
                && let Some(version) = nix::version_at(rev, &result.name, Some(reg))
            {
                found(result.name.clone(), input.channel(), version);
            }
        }
    }
}

/// The newest version of `pkg` on `channel`, if nix-search knows it.
pub fn latest_version(pkg: &str, channel: &str) -> Option<String> {
    match nix_search::search(pkg, channel, None) {
        Ok(results) => results
            .iter()
            .find(|p| p.attribute == pkg)
            .or_else(|| {
                results
                    .iter()
                    .find(|p| p.attribute.ends_with(&format!(".{}", pkg)))
            })
            .and_then(|p| p.version.clone()),
        Err(e) => {
            crate::log::output(format!("Update Check Error ({})", pkg), e.to_string());
            None
        }
    }
}
