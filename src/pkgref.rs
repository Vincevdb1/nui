use crate::backend::system::SystemInfo;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// The nixpkgs the system is built from, at this revision.
    System(String),
    /// nixpkgs at a specific revision.
    Nixpkgs(String),
    /// Whatever the `nixpkgs` registry entry points to.
    Registry,
}

/// A package added to an ad-hoc shell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageRef {
    pub source: Source,
    pub attr: String,
    pub version: Option<String>,
}

impl PackageRef {
    pub fn new(source: Source, attr: impl Into<String>, version: Option<String>) -> Self {
        Self {
            source,
            attr: attr.into(),
            version,
        }
    }

    pub fn rev(&self) -> Option<&str> {
        match &self.source {
            Source::System(rev) | Source::Nixpkgs(rev) => Some(rev),
            Source::Registry => None,
        }
    }

    /// The argument `nix shell` takes for this package.
    pub fn installable(&self, system: &SystemInfo) -> String {
        match &self.source {
            Source::System(rev) => {
                if let Some(path) = system
                    .nixpkgs_path
                    .as_ref()
                    .filter(|p| std::path::Path::new(p).exists())
                {
                    format!("path:{}#{}", path, self.attr)
                } else if let Some(version) = &system.nixpkgs_version {
                    format!("nixpkgs/{}#{}", channel_for(version), self.attr)
                } else {
                    format!("github:NixOS/nixpkgs/{}#{}", rev, self.attr)
                }
            }
            Source::Nixpkgs(rev) => format!("github:NixOS/nixpkgs/{}#{}", rev, self.attr),
            Source::Registry => format!("nixpkgs#{}", self.attr),
        }
    }

    /// A compact form for the shell's `name`, with revisions shortened.
    pub fn short_name(&self) -> String {
        let short = |rev: &str| rev.chars().take(7).collect::<String>();
        let base = match &self.source {
            Source::System(rev) => format!("{}#{}", short(rev), self.attr),
            Source::Nixpkgs(rev) => format!("nixpkgs/{}#{}", short(rev), self.attr),
            Source::Registry => self.attr.clone(),
        };
        match &self.version {
            Some(v) => format!("{}@{}", base, v),
            None => base,
        }
    }
}

fn channel_for(nixpkgs_version: &str) -> String {
    if nixpkgs_version.contains("pre") || nixpkgs_version.contains("unstable") {
        return "nixpkgs-unstable".to_string();
    }
    let parts: Vec<&str> = nixpkgs_version.split('.').collect();
    if parts.len() >= 2 {
        format!("nixos-{}", parts[..2].join("."))
    } else {
        "nixpkgs-unstable".to_string()
    }
}

impl fmt::Display for PackageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Source::System(rev) => write!(f, "system/{}#", rev)?,
            Source::Nixpkgs(rev) => write!(f, "nixpkgs/{}#", rev)?,
            Source::Registry => {}
        }
        write!(f, "{}", self.attr)?;
        if let Some(v) = &self.version {
            write!(f, "@{}", v)?;
        }
        Ok(())
    }
}
