use crate::flake::model::ShellTarget;
use crate::flake::parse::{attr_segments, binding_path, binding_value};
use rnix::{Root, SyntaxKind, SyntaxNode, TextRange};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Where nix files are read from: the filesystem, or a map in tests.
pub trait Sources {
    fn read(&self, path: &Path) -> Option<String>;
}

pub struct Fs;

impl Sources for Fs {
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

impl Sources for HashMap<PathBuf, String> {
    fn read(&self, path: &Path) -> Option<String> {
        self.get(path).cloned()
    }
}

/// The attribute set given to `mkShell` for a devShell.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub file: PathBuf,
    pub set: TextRange,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LocateError {
    #[error("nui could not find the shell in the source")]
    NotFound,
    #[error("nui found {0} possible definitions of the shell")]
    Ambiguous(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Step<'a> {
    Attr(&'a str),
    System,
}

const MAX_DEPTH: usize = 64;

struct Walk<'a> {
    sources: &'a dyn Sources,
    system: &'a str,
    found: Vec<Location>,
}

/// Finds the one shell set for `target`, following functions, `let`s and imports.
pub fn locate(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
) -> Result<Location, LocateError> {
    let src = sources.read(flake).ok_or(LocateError::NotFound)?;
    let root = Root::parse(&src).syntax();
    let Some(outputs) = root
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET)
        .and_then(|set| {
            set.children()
                .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
                .find(|c| binding_path(c).as_deref() == Some("outputs"))
        })
        .and_then(|b| binding_value(&b))
    else {
        return Err(LocateError::NotFound);
    };

    let mut walk = Walk {
        sources,
        system: &target.system,
        found: Vec::new(),
    };
    let path = [
        Step::Attr("devShells"),
        Step::System,
        Step::Attr(&target.name),
    ];
    walk.node(flake, &outputs, &path, false, 0);
    walk.found.dedup();
    match walk.found.len() {
        0 => Err(LocateError::NotFound),
        1 => Ok(walk.found.remove(0)),
        n => Err(LocateError::Ambiguous(n)),
    }
}

/// The node at `range` in `root`, of the given kind.
pub fn node_at(root: &SyntaxNode, range: TextRange, kind: SyntaxKind) -> Option<SyntaxNode> {
    root.descendants()
        .find(|n| n.kind() == kind && n.text_range() == range)
}

impl Walk<'_> {
    fn node(&mut self, file: &Path, node: &SyntaxNode, path: &[Step], applied: bool, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let depth = depth + 1;
        if path.is_empty() {
            return self.shell(file, node, depth);
        }
        match node.kind() {
            SyntaxKind::NODE_LAMBDA
            | SyntaxKind::NODE_LET_IN
            | SyntaxKind::NODE_WITH
            | SyntaxKind::NODE_PAREN
            | SyntaxKind::NODE_ROOT => {
                if let Some(body) = node.children().last() {
                    self.node(file, &body, path, applied, depth);
                }
            }
            SyntaxKind::NODE_APPLY => {
                if let Some(import) = imported_file(file, node) {
                    return self.file(&import, path, applied, depth);
                }
                if let Some(arg) = node.children().last() {
                    self.node(file, &arg, path, true, depth);
                }
            }
            SyntaxKind::NODE_BIN_OP if has_token(node, SyntaxKind::TOKEN_UPDATE) => {
                for side in node.children() {
                    self.node(file, &side, path, applied, depth);
                }
            }
            SyntaxKind::NODE_IDENT => {
                if let Some(value) = resolve_ident(node) {
                    self.node(file, &value, path, applied, depth);
                }
            }
            SyntaxKind::NODE_ATTR_SET => self.set(file, node, path, applied, depth),
            _ => {}
        }
    }

    fn set(&mut self, file: &Path, set: &SyntaxNode, path: &[Step], applied: bool, depth: usize) {
        for binding in set
            .children()
            .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        {
            let (Some(attrpath), Some(value)) = (attrpath(&binding), binding_value(&binding))
            else {
                continue;
            };
            let segments = attr_segments(&attrpath);

            // flake-parts: `perSystem` holds `devShells` for an implicit system.
            if segments == ["perSystem"] && path.first() == Some(&Step::Attr("devShells")) {
                let rest: Vec<Step> = std::iter::once(path[0])
                    .chain(path[2..].iter().copied())
                    .collect();
                self.node(file, &value, &rest, applied, depth);
                continue;
            }

            for rest in self.matches(&segments, path, applied) {
                self.node(file, &value, rest, applied, depth);
            }
        }
    }

    /// Remaining paths after `segments`; the system may be absent below an application.
    fn matches<'p, 's>(
        &self,
        segments: &[String],
        path: &'p [Step<'s>],
        applied: bool,
    ) -> Vec<&'p [Step<'s>]> {
        let mut result = Vec::new();
        self.match_from(segments, path, applied, &mut result);
        result
    }

    fn match_from<'p, 's>(
        &self,
        segments: &[String],
        path: &'p [Step<'s>],
        applied: bool,
        result: &mut Vec<&'p [Step<'s>]>,
    ) {
        let Some((segment, rest_segments)) = segments.split_first() else {
            result.push(path);
            return;
        };
        let Some((step, rest_path)) = path.split_first() else {
            return;
        };
        if step_matches(segment, *step, self.system) {
            self.match_from(rest_segments, rest_path, applied, result);
        }
        if applied && *step == Step::System {
            self.match_from(segments, rest_path, applied, result);
        }
    }

    fn file(&mut self, file: &Path, path: &[Step], applied: bool, depth: usize) {
        let Some(src) = self.sources.read(file) else {
            return;
        };
        let root = Root::parse(&src).syntax();
        self.node(file, &root, path, applied, depth);
    }

    /// At the shell value: unwraps bindings and imports down to the set given to `mkShell`.
    fn shell(&mut self, file: &Path, node: &SyntaxNode, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let depth = depth + 1;
        match node.kind() {
            SyntaxKind::NODE_IDENT => {
                if let Some(value) = resolve_ident(node) {
                    self.shell(file, &value, depth);
                }
            }
            SyntaxKind::NODE_ROOT
            | SyntaxKind::NODE_PAREN
            | SyntaxKind::NODE_LAMBDA
            | SyntaxKind::NODE_LET_IN => {
                if let Some(body) = node.children().last() {
                    self.shell(file, &body, depth);
                }
            }
            SyntaxKind::NODE_APPLY => match imported_file(file, node) {
                Some(import) => {
                    if let Some(src) = self.sources.read(&import) {
                        let root = Root::parse(&src).syntax();
                        self.shell(&import, &root, depth);
                    }
                }
                None => self.push(file, shell_attr_set(node)),
            },
            _ => self.push(file, shell_attr_set(node)),
        }
    }

    fn push(&mut self, file: &Path, set: Option<SyntaxNode>) {
        if let Some(set) = set {
            self.found.push(Location {
                file: file.to_path_buf(),
                set: set.text_range(),
            });
        }
    }
}

fn step_matches(segment: &str, step: Step, system: &str) -> bool {
    match step {
        Step::Attr(name) => segment.trim_matches('"') == name,
        Step::System => match_segment(segment, system, system),
    }
}

pub(crate) fn attrpath(binding: &SyntaxNode) -> Option<SyntaxNode> {
    binding
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
}

/// Matches an attribute segment, treating `system` and `${system}` as the target system.
pub fn match_segment(actual: &str, expected: &str, system: &str) -> bool {
    let actual = actual.trim_matches('"');
    if actual == expected {
        return true;
    }
    if expected != system {
        return false;
    }
    actual == "system"
        || actual
            .strip_prefix("${")
            .and_then(|s| s.strip_suffix('}'))
            .is_some_and(|inner| inner.trim() == "system")
}

/// The set in `mkShell { ... }`, `with pkgs; mkShell { ... }` or a plain set.
pub fn shell_attr_set(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        SyntaxKind::NODE_APPLY => node
            .children()
            .last()
            .filter(|c| c.kind() == SyntaxKind::NODE_ATTR_SET),
        SyntaxKind::NODE_ATTR_SET => Some(node.clone()),
        SyntaxKind::NODE_WITH | SyntaxKind::NODE_PAREN => shell_attr_set(&node.children().last()?),
        _ => None,
    }
}

fn has_token(node: &SyntaxNode, kind: SyntaxKind) -> bool {
    node.children_with_tokens().any(|c| c.kind() == kind)
}

/// The value bound to an identifier by an enclosing `let`, unless a lambda shadows it.
fn resolve_ident(ident: &SyntaxNode) -> Option<SyntaxNode> {
    let name = ident.text().to_string();
    let mut child = ident.clone();
    for ancestor in ident.ancestors().skip(1) {
        match ancestor.kind() {
            SyntaxKind::NODE_LET_IN => {
                let binding = ancestor
                    .children()
                    .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
                    .find(|b| binding_path(b).as_deref() == Some(name.as_str()));
                // A binding must not resolve to itself.
                if let Some(value) = binding.and_then(|b| binding_value(&b))
                    && !value.text_range().contains_range(child.text_range())
                {
                    return Some(value);
                }
            }
            SyntaxKind::NODE_LAMBDA if lambda_binds(&ancestor, &name) => return None,
            _ => {}
        }
        child = ancestor;
    }
    None
}

fn lambda_binds(lambda: &SyntaxNode, name: &str) -> bool {
    let Some(param) = lambda.children().next() else {
        return false;
    };
    param.descendants().any(|n| {
        n.kind() == SyntaxKind::NODE_IDENT
            && n.text() == name
            && n.parent().is_some_and(|p| {
                matches!(
                    p.kind(),
                    SyntaxKind::NODE_IDENT_PARAM
                        | SyntaxKind::NODE_PAT_ENTRY
                        | SyntaxKind::NODE_PAT_BIND
                )
            })
    })
}

/// The file read by `import ./x.nix`, `import ./x.nix { }` or `callPackage ./x.nix { }`.
pub fn imported_file(file: &Path, apply: &SyntaxNode) -> Option<PathBuf> {
    let mut head = apply.clone();
    let mut args = Vec::new();
    while head.kind() == SyntaxKind::NODE_APPLY {
        let mut children = head.children();
        let function = children.next()?;
        args.push(children.last()?);
        head = function;
    }
    args.reverse();
    let name = head.text().to_string();
    let name = name.rsplit('.').next().unwrap_or("");
    if !matches!(name, "import" | "callPackage") {
        return None;
    }
    let path = args
        .first()
        .filter(|a| a.kind() == SyntaxKind::NODE_PATH_REL)?;
    Some(relative_to(file, path.text().to_string().trim()))
}

/// `rel` resolved against the directory of `file`, without `.` and `..` parts.
pub fn relative_to(file: &Path, rel: &str) -> PathBuf {
    let joined = file.parent().unwrap_or(Path::new("")).join(rel);
    let mut result = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir if result.file_name().is_some() => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    if result.extension().is_none() {
        result.push("default.nix");
    }
    result
}

/// Every file `flake` reaches through `import` or `callPackage`, `flake` first.
pub fn reachable_files(sources: &dyn Sources, flake: &Path) -> Vec<(PathBuf, String)> {
    let mut result: Vec<(PathBuf, String)> = Vec::new();
    let mut pending = vec![flake.to_path_buf()];
    while let Some(file) = pending.pop() {
        if result.iter().any(|(f, _)| *f == file) {
            continue;
        }
        let Some(src) = sources.read(&file) else {
            continue;
        };
        let root = Root::parse(&src).syntax();
        pending.extend(
            root.descendants()
                .filter(|n| n.kind() == SyntaxKind::NODE_APPLY)
                .filter_map(|n| imported_file(&file, &n)),
        );
        result.push((file, src));
    }
    result
}
