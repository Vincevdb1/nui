use crate::flake::model::ShellTarget;
use crate::flake::parse::{PACKAGE_LIST_ATTRS, binding_path, binding_value, package_list};
use rnix::{SyntaxKind, SyntaxNode, SyntaxToken};

pub fn attr_segments(attrpath: &SyntaxNode) -> Vec<String> {
    attrpath
        .children_with_tokens()
        .filter(|c| {
            !matches!(
                c.kind(),
                SyntaxKind::TOKEN_DOT | SyntaxKind::TOKEN_WHITESPACE
            )
        })
        .map(|c| c.to_string().trim().to_string())
        .collect()
}

fn attrpath(binding: &SyntaxNode) -> Option<SyntaxNode> {
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

fn find_path(node: &SyntaxNode, path: &[&str], system: &str) -> Option<SyntaxNode> {
    let Some(&target) = path.first() else {
        return Some(node.clone());
    };

    if node.kind() == SyntaxKind::NODE_LAMBDA
        && let Some(body) = node.children().find(|c| {
            matches!(
                c.kind(),
                SyntaxKind::NODE_ATTR_SET | SyntaxKind::NODE_LET_IN | SyntaxKind::NODE_WITH
            )
        })
    {
        return find_path(&body, path, system);
    }

    if node.kind() == SyntaxKind::NODE_ATTR_SET {
        for child in node
            .children()
            .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        {
            let Some(attrpath) = attrpath(&child) else {
                continue;
            };
            let segments = attr_segments(&attrpath);
            if segments.is_empty() || !match_segment(&segments[0], target, system) {
                continue;
            }
            let matched = segments
                .iter()
                .zip(path)
                .take_while(|(seg, expected)| match_segment(seg, expected, system))
                .count();
            if matched == segments.len() {
                let val = binding_value(&child)?;
                return find_path(&val, &path[matched..], system);
            }
        }
    }

    if matches!(node.kind(), SyntaxKind::NODE_LET_IN | SyntaxKind::NODE_WITH)
        && let Some(body) = node.children().find(|c| {
            !matches!(
                c.kind(),
                SyntaxKind::NODE_ATTRPATH
                    | SyntaxKind::TOKEN_COMMENT
                    | SyntaxKind::TOKEN_WHITESPACE
                    | SyntaxKind::TOKEN_LET
                    | SyntaxKind::TOKEN_IN
                    | SyntaxKind::TOKEN_WITH
                    | SyntaxKind::NODE_IDENT
                    | SyntaxKind::TOKEN_SEMICOLON
            )
        })
    {
        return find_path(&body, path, system);
    }

    if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE
        && let Some(val) = binding_value(node)
    {
        return find_path(&val, path, system);
    }

    None
}

fn find_shell_node(root: &SyntaxNode, target: &ShellTarget) -> Option<SyntaxNode> {
    let system = target.system.as_str();
    let name = target.name.as_str();
    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
    {
        let Some(attrpath) = attrpath(&node) else {
            continue;
        };
        let segments = attr_segments(&attrpath);
        if segments.first().map(String::as_str) != Some("devShells") {
            continue;
        }
        let val = binding_value(&node)?;
        let found = match segments.len() {
            1 => find_path(&val, &[system, name], system),
            2 if match_segment(&segments[1], system, system) => find_path(&val, &[name], system),
            n if n >= 3 && match_segment(&segments[1], system, system) && segments[2] == name => {
                Some(val)
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn shell_attr_set(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        SyntaxKind::NODE_APPLY => node
            .children()
            .find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET),
        SyntaxKind::NODE_ATTR_SET => Some(node.clone()),
        SyntaxKind::NODE_WITH => {
            let body = node.children().find(|c| {
                !matches!(
                    c.kind(),
                    SyntaxKind::TOKEN_WITH
                        | SyntaxKind::NODE_IDENT
                        | SyntaxKind::TOKEN_SEMICOLON
                        | SyntaxKind::TOKEN_WHITESPACE
                        | SyntaxKind::TOKEN_COMMENT
                )
            })?;
            shell_attr_set(&body)
        }
        _ => None,
    }
}

/// The attribute set passed to `mkShell` for the target devShell.
pub fn shell_set(root: &SyntaxNode, target: &ShellTarget) -> Option<SyntaxNode> {
    shell_attr_set(&find_shell_node(root, target)?)
}

/// Every `packages`/`buildInputs`/`nativeBuildInputs` list directly inside a shell set.
pub fn package_lists(shell_set: &SyntaxNode) -> Vec<SyntaxNode> {
    shell_set
        .children()
        .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .filter(|c| binding_path(c).is_some_and(|p| PACKAGE_LIST_ATTRS.contains(&p.as_str())))
        .filter_map(|c| binding_value(&c).and_then(package_list))
        .collect()
}

/// List items whose text equals `pkg`, in source order.
pub fn list_items<'a>(list: &SyntaxNode, pkg: &'a str) -> impl Iterator<Item = SyntaxNode> + 'a {
    list.children().filter(move |c| {
        matches!(c.kind(), SyntaxKind::NODE_SELECT | SyntaxKind::NODE_IDENT)
            && c.to_string().trim() == pkg
    })
}

pub fn replace_in_lists(lists: &[SyntaxNode], old_pkg: &str, new_pkg: &str, src: &str) -> String {
    let mut ranges: Vec<_> = lists
        .iter()
        .flat_map(|list| list_items(list, old_pkg).map(|item| item.text_range()))
        .collect();
    ranges.sort_by_key(|r| r.start());
    let mut result = src.to_string();
    for range in ranges.into_iter().rev() {
        result.replace_range(
            usize::from(range.start())..usize::from(range.end()),
            new_pkg,
        );
    }
    result
}

pub fn last_token(node: &SyntaxNode, text: &str) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|c| c.into_token())
        .filter(|t| t.text() == text)
        .last()
}

pub fn whitespace_before(token: &SyntaxToken) -> Option<SyntaxToken> {
    token
        .prev_sibling_or_token()
        .and_then(|p| p.into_token())
        .filter(|t| t.kind() == SyntaxKind::TOKEN_WHITESPACE)
}

pub fn last_line(ws: Option<&SyntaxToken>) -> String {
    ws.and_then(|t| t.text().lines().last().map(str::to_string))
        .unwrap_or_default()
}

/// Indentation of the first binding in a set, or `fallback` when it has none.
pub fn binding_indent(set: &SyntaxNode, fallback: &str) -> String {
    set.children()
        .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .and_then(|first| {
            first
                .prev_sibling_or_token()
                .filter(|p| p.kind() == SyntaxKind::TOKEN_WHITESPACE)
                .and_then(|p| p.to_string().lines().last().map(str::to_string))
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Replaces the closing brace of `set` (and the whitespace before it) with `entry` + `}`.
pub fn append_to_set(set: &SyntaxNode, entry: &str, src: &str) -> Option<String> {
    let close = last_token(set, "}")?;
    let ws = whitespace_before(&close);
    let start = ws.as_ref().unwrap_or(&close).text_range().start();
    let indent = binding_indent(set, "    ");
    let replacement = format!("\n{}{}\n{}}}", indent, entry, last_line(ws.as_ref()));
    let mut result = src.to_string();
    result.replace_range(
        usize::from(start)..usize::from(close.text_range().end()),
        &replacement,
    );
    Some(result)
}

/// The `{ self, nixpkgs, ... }` pattern of the outputs function.
pub fn outputs_pattern(root: &SyntaxNode) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .find(|n| attrpath(n).is_some_and(|p| p.text().to_string().trim() == "outputs"))?
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_LAMBDA)?
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_PATTERN)
}

/// The `inputs = { ... }` set.
pub fn inputs_set(root: &SyntaxNode) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .filter(|n| attrpath(n).is_some_and(|p| p.text().to_string().trim() == "inputs"))
        .find_map(|n| n.children().find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET))
}
