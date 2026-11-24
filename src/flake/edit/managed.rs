use super::ast::{append_to_set, package_bindings};
use super::packages::{append_to_list, inputs_bind};
use super::pin::bind_inputs;
use super::{Changes, EditError, Overlay};
use crate::flake::locate::{Location, Sources};
use crate::flake::parse::{self, binding_value};
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::path::{Component, Path, PathBuf};

const HEADER: &str = "# Managed by nui.\n{ pkgs, inputs ? { }, ... }:\nwith pkgs;\n[\n]\n";

/// `nui/<name>.nix` next to flake.nix; one file serves the shell on every system.
pub fn managed_path(flake: &Path, name: &str) -> PathBuf {
    flake
        .parent()
        .unwrap_or(Path::new(""))
        .join("nui")
        .join(format!("{}.nix", name))
}

/// Whether the located shell set already imports the managed file.
pub(super) fn is_imported(set_text: &str, name: &str) -> bool {
    set_text.contains(&format!("nui/{}.nix", name))
}

/// The root list of a managed file.
pub(super) fn managed_lists(src: &str) -> Vec<SyntaxNode> {
    let root = Root::parse(src).syntax();
    let Some(body) = root
        .children()
        .last()
        .filter(|l| l.kind() == SyntaxKind::NODE_LAMBDA)
        .and_then(|l| l.children().last())
    else {
        return Vec::new();
    };
    parse::package_lists(&body)
        .into_iter()
        .map(|(list, _)| list)
        .collect()
}

pub(super) fn add_to_managed(files: &mut Overlay, path: &Path, pkg: &str) -> Result<(), EditError> {
    let src = files.read(path).unwrap_or_else(|| HEADER.to_string());
    let list = managed_lists(&src)
        .into_iter()
        .next()
        .ok_or_else(|| EditError::Unreadable(path.to_path_buf()))?;
    // The list starts at column 0, so its first item sets the indentation.
    let edited = match list.children().next() {
        None => {
            let range = list.text_range();
            let mut result = src.clone();
            result.replace_range(
                usize::from(range.start())..usize::from(range.end()),
                &format!("[\n  {}\n]", pkg),
            );
            Some(result)
        }
        Some(_) => append_to_list(&list, pkg, &src),
    }
    .ok_or_else(|| EditError::Unreadable(path.to_path_buf()))?;
    files.set(path, edited);
    Ok(())
}

/// The import expression, written in `from` and resolved against its directory.
fn import_expr(from: &Path, managed: &Path, args: &str) -> String {
    format!("import {} {{ {} }}", relative_path(from, managed), args)
}

/// Arguments to the managed file, and whether flake.nix must bind `inputs` for them.
/// `inputs` is passed only from flake.nix, where the outputs pattern can bind it.
fn import_args(
    files: &Overlay,
    flake: &Path,
    set_file: &Path,
) -> Result<(String, bool), EditError> {
    if set_file != flake {
        return Ok(("inherit pkgs;".to_string(), false));
    }
    Ok(match inputs_bind(&files.get(flake)?).as_deref() {
        None => ("inherit pkgs inputs;".to_string(), true),
        Some("inputs") => ("inherit pkgs inputs;".to_string(), false),
        Some(bind) => (format!("inherit pkgs; inputs = {};", bind), false),
    })
}

/// Sends the shell's package attribute to the managed file, which then holds `pkg`.
pub(super) fn add_with_import(
    files: &mut Overlay,
    flake: &Path,
    name: &str,
    location: &Location,
    pkg: &str,
) -> Result<(), EditError> {
    let managed = managed_path(flake, name);
    add_to_managed(files, &managed, pkg)?;
    let (args, bind) = import_args(files, flake, &location.file)?;
    let import = import_expr(&location.file, &managed, &args);

    let src = files.get(&location.file)?;
    let set = super::shell_node(&src, location)
        .ok_or_else(|| EditError::Unreadable(location.file.clone()))?;
    let edited = match package_bindings(&set).find_map(|b| binding_value(&b)) {
        Some(value) => {
            let range = value.text_range();
            let text = match needs_parens(&value) {
                true => format!("({}) ++ {}", value, import),
                false => format!("{} ++ {}", value, import),
            };
            let mut result = src.clone();
            result.replace_range(usize::from(range.start())..usize::from(range.end()), &text);
            result
        }
        None => append_to_set(&set, &format!("packages = {};", import), &src)
            .ok_or_else(|| EditError::Unreadable(location.file.clone()))?,
    };
    files.set(&location.file, edited);
    // The pattern comes before the set, so binding it last keeps the set's range valid.
    if bind {
        let flake_src = files.get(flake)?;
        files.set(flake, bind_inputs(&flake_src));
    }
    Ok(())
}

fn needs_parens(value: &SyntaxNode) -> bool {
    match value.kind() {
        SyntaxKind::NODE_IDENT
        | SyntaxKind::NODE_SELECT
        | SyntaxKind::NODE_LIST
        | SyntaxKind::NODE_APPLY
        | SyntaxKind::NODE_PAREN
        | SyntaxKind::NODE_WITH => false,
        SyntaxKind::NODE_BIN_OP => !value
            .children_with_tokens()
            .any(|c| c.kind() == SyntaxKind::TOKEN_CONCAT),
        _ => true,
    }
}

/// Creates the managed file for a shell nui cannot find; the user adds the returned line.
pub fn create_managed(
    sources: &dyn Sources,
    flake: &Path,
    name: &str,
    pkg: &str,
) -> Result<(Changes, String), EditError> {
    let mut files = Overlay::new(sources);
    let managed = managed_path(flake, name);
    add_to_managed(&mut files, &managed, pkg)?;
    let line = format!(
        "packages = {};",
        import_expr(flake, &managed, "inherit pkgs inputs;")
    );
    Ok((files.changes(), line))
}

/// `to` as a `./` or `../` path from the directory of `from`.
fn relative_path(from: &Path, to: &Path) -> String {
    let parts = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect()
    };
    let from = parts(from.parent().unwrap_or(Path::new("")));
    let to = parts(to);
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut result: Vec<String> = vec!["..".to_string(); from.len() - common];
    result.extend(to[common..].iter().cloned());
    match result.first().map(String::as_str) {
        Some("..") => result.join("/"),
        _ => format!("./{}", result.join("/")),
    }
}
