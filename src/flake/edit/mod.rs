mod ast;
mod inputs;
mod managed;
mod packages;
mod pin;

pub use inputs::{add_input, remove_input};
pub use managed::{create_managed, managed_path};
pub use packages::{add_package, remove_packages, shell_packages};
pub use pin::{pin_package, unpin_package};

use crate::flake::locate::{LocateError, Location, Sources, locate, node_at};
use crate::flake::model::ShellTarget;
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Files an edit changed, with their new content.
pub type Changes = Vec<(PathBuf, String)>;

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("could not find devShell {}.{} in flake.nix", .0.system, .0.name)]
    ShellNotFound(ShellTarget),
    #[error("found {1} definitions of devShell {system}.{name}", system = .0.system, name = .0.name)]
    AmbiguousShell(ShellTarget, usize),
    #[error("could not find source input for {0}")]
    NoSourceInput(String),
    #[error("input {0} has no revision in lock file. Run 'nix flake lock' first.")]
    Unlocked(String),
    #[error("could not find package {0} in flake.nix")]
    PackageNotFound(String),
    #[error("package {0} is not pinned")]
    NotPinned(String),
    #[error("invalid pinned package reference: {0}")]
    InvalidPinnedRef(String),
    #[error("could not read {0}")]
    Unreadable(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl EditError {
    fn locate(target: &ShellTarget, error: LocateError) -> Self {
        match error {
            LocateError::NotFound => Self::ShellNotFound(target.clone()),
            LocateError::Ambiguous(n) => Self::AmbiguousShell(target.clone(), n),
        }
    }
}

/// Writes each changed file, creating its directory when needed.
pub fn apply(changes: &Changes) -> Result<(), EditError> {
    for (path, content) in changes {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// A pure text edit of one file, as `Changes`.
pub fn edit_file(
    sources: &dyn Sources,
    path: &Path,
    edit: impl FnOnce(&str) -> String,
) -> Result<Changes, EditError> {
    let mut files = Overlay::new(sources);
    let src = files.get(path)?;
    files.set(path, edit(&src));
    Ok(files.changes())
}

/// Files read once and edited in memory; reads see earlier edits.
struct Overlay<'a> {
    sources: &'a dyn Sources,
    files: BTreeMap<PathBuf, (Option<String>, String)>,
}

impl<'a> Overlay<'a> {
    fn new(sources: &'a dyn Sources) -> Self {
        Self {
            sources,
            files: BTreeMap::new(),
        }
    }

    fn get(&self, path: &Path) -> Result<String, EditError> {
        self.read(path)
            .ok_or_else(|| EditError::Unreadable(path.to_path_buf()))
    }

    fn set(&mut self, path: &Path, content: String) {
        let original = self.sources.read(path);
        self.files
            .entry(path.to_path_buf())
            .or_insert((original, String::new()))
            .1 = content;
    }

    fn changes(self) -> Changes {
        self.files
            .into_iter()
            .filter(|(_, (original, content))| original.as_ref() != Some(content))
            .map(|(path, (_, content))| (path, content))
            .collect()
    }
}

impl Sources for Overlay<'_> {
    fn read(&self, path: &Path) -> Option<String> {
        match self.files.get(path) {
            Some((_, content)) => Some(content.clone()),
            None => self.sources.read(path),
        }
    }
}

fn find_shell(files: &Overlay, flake: &Path, target: &ShellTarget) -> Result<Location, EditError> {
    locate(files, flake, target).map_err(|e| EditError::locate(target, e))
}

/// The located shell set, parsed from its file.
fn shell_node(src: &str, location: &Location) -> Option<SyntaxNode> {
    let root = Root::parse(src).syntax();
    node_at(&root, location.set, SyntaxKind::NODE_ATTR_SET)
}

/// Where a shell's package lists live: its set, the managed file it imports, or both.
struct ShellLists {
    set: Option<Location>,
    managed: Option<PathBuf>,
}

/// The shell's lists; without a located set, an existing managed file stands in for it.
fn shell_lists(
    files: &Overlay,
    flake: &Path,
    target: &ShellTarget,
) -> Result<ShellLists, EditError> {
    let managed = managed_path(flake, &target.name);
    let has_managed = files.read(&managed).is_some();
    match find_shell(files, flake, target) {
        Ok(location) => {
            let src = files.get(&location.file)?;
            let set_text = &src[usize::from(location.set.start())..usize::from(location.set.end())];
            let imported = has_managed && managed::is_imported(set_text, &target.name);
            Ok(ShellLists {
                set: Some(location),
                managed: imported.then_some(managed),
            })
        }
        Err(_) if has_managed => Ok(ShellLists {
            set: None,
            managed: Some(managed),
        }),
        Err(e) => Err(e),
    }
}

impl ShellLists {
    fn files(&self) -> Vec<PathBuf> {
        self.set
            .iter()
            .map(|l| l.file.clone())
            .chain(self.managed.clone())
            .collect()
    }

    fn lists(&self, path: &Path, src: &str) -> Vec<SyntaxNode> {
        let mut lists = Vec::new();
        if let Some(location) = self.set.as_ref().filter(|l| l.file == path)
            && let Some(set) = shell_node(src, location)
        {
            lists.extend(ast::package_lists(&set));
        }
        if self.managed.as_deref() == Some(path) {
            lists.extend(managed::managed_lists(src));
        }
        lists
    }
}
