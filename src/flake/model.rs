#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    pub name: String,
    pub url: String,
    pub branch: Option<String>,
    pub rev: Option<String>,
}

impl Input {
    pub fn is_nixpkgs(&self) -> bool {
        self.url.contains("nixpkgs")
    }

    /// The nixpkgs channel this input tracks, used to pick the matching search channel.
    pub fn upstream_channel(&self) -> String {
        if self.is_nixpkgs() {
            for part in self.url.split('/') {
                if part.starts_with("nixos-") || part.starts_with("nixpkgs-") {
                    return part.strip_suffix(".tar.gz").unwrap_or(part).to_string();
                }
            }
        }
        "nixos-unstable".to_string()
    }

    pub fn channel(&self) -> String {
        self.branch
            .clone()
            .or_else(|| self.rev.clone())
            .unwrap_or_else(|| self.upstream_channel())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    DevShell,
    Darwin,
    Home,
    Nixos,
    Packages,
    LegacyPackages,
}

impl OutputKind {
    pub fn from_attr(attr: &str) -> Option<Self> {
        Some(match attr {
            "devShells" => Self::DevShell,
            "darwinConfigurations" => Self::Darwin,
            "homeConfigurations" => Self::Home,
            "nixosConfigurations" => Self::Nixos,
            "packages" => Self::Packages,
            "legacyPackages" => Self::LegacyPackages,
            _ => return None,
        })
    }

    pub fn attr(self) -> &'static str {
        match self {
            Self::DevShell => "devShells",
            Self::Darwin => "darwinConfigurations",
            Self::Home => "homeConfigurations",
            Self::Nixos => "nixosConfigurations",
            Self::Packages => "packages",
            Self::LegacyPackages => "legacyPackages",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DevShell => "SHELL",
            Self::Nixos => "OS",
            Self::Home => "HM",
            Self::Darwin => "DRWN",
            Self::Packages | Self::LegacyPackages => "???",
        }
    }

    /// Outputs that `nix flake show` nests per system.
    pub fn is_per_system(self) -> bool {
        matches!(self, Self::DevShell | Self::Packages | Self::LegacyPackages)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub kind: OutputKind,
    pub path: String,
    pub name: Option<String>,
    pub content: Option<String>,
}

/// The devShell an edit applies to; only devShells are editable.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellTarget {
    pub system: String,
    pub name: String,
}

impl Output {
    pub fn shell_target(&self) -> Option<ShellTarget> {
        if self.kind != OutputKind::DevShell {
            return None;
        }
        let (system, name) = self.path.split_once('.')?;
        Some(ShellTarget {
            system: system.to_string(),
            name: name.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub version: String,
    pub is_unfree: bool,
    pub source: Option<String>,
    /// The list item as written in flake.nix, e.g. `pkgs.hello`.
    pub attr: Option<String>,
}

impl Package {
    pub fn qualified_name(&self) -> String {
        self.attr.clone().unwrap_or_else(|| self.name.clone())
    }

    pub fn is_pinned(&self) -> bool {
        self.attr.as_deref().is_some_and(|a| a.starts_with("inputs."))
    }
}
