use super::ast::{
    append_to_set, last_line, last_token, list_items, outputs_pattern, package_lists,
    whitespace_before,
};
use super::inputs::remove_input;
use super::{Changes, EditError, Overlay, find_shell, shell_node};
use crate::flake::locate::{Sources, reachable_files};
use crate::flake::model::ShellTarget;
use crate::flake::parse;
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::path::Path;

/// Appends to the shell's first package list, creating `packages` when there is none.
pub fn add_package(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
    pkg: &str,
) -> Result<Changes, EditError> {
    let mut files = Overlay::new(sources);
    let location = find_shell(&files, flake, target)?;
    let src = files.get(&location.file)?;
    let set =
        shell_node(&src, &location).ok_or_else(|| EditError::ShellNotFound(target.clone()))?;
    let edited = match package_lists(&set).first() {
        Some(list) => append_to_list(list, pkg, &src),
        None => append_to_set(&set, &format!("packages = [ {} ];", pkg), &src),
    }
    .ok_or_else(|| EditError::ShellNotFound(target.clone()))?;
    files.set(&location.file, edited);
    Ok(files.changes())
}

pub(super) fn append_to_list(list: &SyntaxNode, pkg: &str, src: &str) -> Option<String> {
    let close = last_token(list, "]")?;
    let ws = whitespace_before(&close);
    let item_indent = list
        .children_with_tokens()
        .find(|c| c.kind() == SyntaxKind::TOKEN_WHITESPACE && c.to_string().contains('\n'))
        .and_then(|c| c.to_string().lines().last().map(str::to_string));

    let entry = match item_indent {
        Some(mut indent) => {
            let closing_indent = last_line(ws.as_ref());
            if indent == closing_indent && !indent.is_empty() {
                indent.push_str("  ");
            }
            format!("\n{}{}\n{}", indent, pkg, closing_indent)
        }
        None if list.children().next().is_none() => format!(" {} ", pkg),
        None => format!(" {}{}", pkg, ws.as_ref().map_or("", |w| w.text())),
    };

    let start = ws.as_ref().unwrap_or(&close).text_range().start();
    let mut result = src.to_string();
    result.replace_range(
        usize::from(start)..usize::from(close.text_range().end()),
        &format!("{}]", entry),
    );
    Some(result)
}

/// Removes the packages from the shell and drops inputs that are no longer referenced.
pub fn remove_packages(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
    pkgs: &[String],
) -> Result<Changes, EditError> {
    let mut files = Overlay::new(sources);
    for pkg in pkgs {
        // Each removal moves the text, so the shell is found again.
        let Ok(location) = find_shell(&files, flake, target) else {
            return Ok(files.changes());
        };
        let src = files.get(&location.file)?;
        if let Some(set) = shell_node(&src, &location) {
            let edited = remove_from_lists(&package_lists(&set), pkg, &src);
            files.set(&location.file, edited);
        }
        if let (Some(input), _) = parse::ref_source(pkg) {
            drop_unused_input(&mut files, flake, input)?;
        }
    }
    Ok(files.changes())
}

/// Removes the input from flake.nix when nothing refers to it any more.
pub(super) fn drop_unused_input(
    files: &mut Overlay,
    flake: &Path,
    input: &str,
) -> Result<(), EditError> {
    let src = files.get(flake)?;
    if parse::inputs(&src, None).iter().any(|i| i.name == input)
        && !input_is_used(files, flake, input)
    {
        files.set(flake, remove_input(&src, input));
    }
    Ok(())
}

pub(super) fn remove_from_lists(lists: &[SyntaxNode], pkg: &str, src: &str) -> String {
    let mut ranges: Vec<_> = lists
        .iter()
        .flat_map(|list| list_items(list, pkg).collect::<Vec<_>>())
        .map(|item| {
            let start = item
                .prev_sibling_or_token()
                .filter(|p| p.kind() == SyntaxKind::TOKEN_WHITESPACE)
                .map(|p| p.text_range().start())
                .unwrap_or(item.text_range().start());
            usize::from(start)..usize::from(item.text_range().end())
        })
        .collect();
    ranges.sort_by_key(|r| r.start);

    let mut result = src.to_string();
    for range in ranges.into_iter().rev() {
        result.replace_range(range, "");
    }
    result
}

/// Whether any file the flake reaches still refers to `input`.
pub(super) fn input_is_used(sources: &dyn Sources, flake: &Path, input: &str) -> bool {
    let bind = sources.read(flake).and_then(|src| inputs_bind(&src));
    reachable_files(sources, flake)
        .iter()
        .any(|(_, src)| refers_to(src, input, bind.as_deref()))
}

/// The name in `name@{ ... }` of the outputs pattern.
pub(super) fn inputs_bind(flake_src: &str) -> Option<String> {
    let root = Root::parse(flake_src).syntax();
    outputs_pattern(&root)?
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_PAT_BIND)?
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_IDENT)
        .map(|i| i.text().to_string())
}

/// A use of `input` as a value, or as `inputs.<input>`; keys and parameters do not count.
fn refers_to(src: &str, input: &str, bind: Option<&str>) -> bool {
    let root = Root::parse(src).syntax();
    root.descendants().any(|n| match n.kind() {
        SyntaxKind::NODE_IDENT => {
            n.text() == input
                && !n.parent().is_some_and(|p| {
                    matches!(
                        p.kind(),
                        SyntaxKind::NODE_ATTRPATH
                            | SyntaxKind::NODE_PAT_ENTRY
                            | SyntaxKind::NODE_IDENT_PARAM
                            | SyntaxKind::NODE_PAT_BIND
                    )
                })
        }
        SyntaxKind::NODE_SELECT => {
            let mut children = n.children();
            let head = children
                .next()
                .filter(|h| h.kind() == SyntaxKind::NODE_IDENT);
            let first = children
                .next()
                .and_then(|path| path.children().next())
                .map(|s| s.text().to_string());
            head.is_some_and(|h| {
                h.text() == "inputs" || Some(h.text().to_string().as_str()) == bind
            }) && first.as_deref() == Some(input)
        }
        _ => false,
    })
}

/// The located shell set as text, for the view.
pub fn shell_source(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
) -> Result<String, EditError> {
    let files = Overlay::new(sources);
    let location = find_shell(&files, flake, target)?;
    let src = files.get(&location.file)?;
    Ok(src[usize::from(location.set.start())..usize::from(location.set.end())].to_string())
}
