use super::EditError;
use super::ast::{
    append_to_set, last_line, last_token, list_items, package_lists, shell_set, whitespace_before,
};
use super::inputs::remove_input;
use crate::flake::model::ShellTarget;
use crate::flake::parse;
use rnix::{Root, SyntaxKind, SyntaxNode};

/// Appends to the shell's first package list, creating `packages` when there is none.
pub fn add_package(src: &str, target: &ShellTarget, pkg: &str) -> Result<String, EditError> {
    let root = Root::parse(src).syntax();
    let set = shell_set(&root, target).ok_or_else(|| EditError::ShellNotFound(target.clone()))?;
    let edited = match package_lists(&set).first() {
        Some(list) => append_to_list(list, pkg, src),
        None => append_to_set(&set, &format!("packages = [ {} ];", pkg), src),
    };
    edited.ok_or_else(|| EditError::ShellNotFound(target.clone()))
}

fn append_to_list(list: &SyntaxNode, pkg: &str, src: &str) -> Option<String> {
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
        None => format!(" {}", pkg),
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
pub fn remove_packages(src: &str, target: &ShellTarget, pkgs: &[String]) -> String {
    let mut result = src.to_string();
    for pkg in pkgs {
        result = remove_from_shell(&result, target, pkg);
        if let (Some(input), _) = parse::ref_source(pkg)
            && parse::inputs(&result, None).iter().any(|i| i.name == input)
            && !input_is_used(&result, input)
        {
            result = remove_input(&result, input);
        }
    }
    result
}

fn remove_from_shell(src: &str, target: &ShellTarget, pkg: &str) -> String {
    let root = Root::parse(src).syntax();
    let Some(set) = shell_set(&root, target) else {
        return src.to_string();
    };
    let mut ranges: Vec<_> = package_lists(&set)
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

/// Whether any output still lists a package from `input`.
pub fn input_is_used(src: &str, input: &str) -> bool {
    parse::outputs(src).iter().any(|output| {
        parse::package_attributes(output.content.as_deref().unwrap_or(""))
            .iter()
            .any(|a| a == input || parse::ref_source(a).0 == Some(input))
    })
}
