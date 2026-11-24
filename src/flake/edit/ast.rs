use crate::flake::locate::attrpath;
use crate::flake::parse::{PACKAGE_LIST_ATTRS, binding_path, binding_value, package_list};
use rnix::{SyntaxKind, SyntaxNode, SyntaxToken};

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

/// Replaces every `old_pkg` item, asking `new_pkg` what to write in each list.
pub fn replace_in_lists(
    lists: &[SyntaxNode],
    old_pkg: &str,
    new_pkg: impl Fn(&SyntaxNode) -> String,
    src: &str,
) -> String {
    let mut edits: Vec<_> = lists
        .iter()
        .flat_map(|list| {
            let replacement = new_pkg(list);
            list_items(list, old_pkg).map(move |item| (item.text_range(), replacement.clone()))
        })
        .collect();
    edits.sort_by_key(|(range, _)| range.start());
    let mut result = src.to_string();
    for (range, replacement) in edits.into_iter().rev() {
        result.replace_range(
            usize::from(range.start())..usize::from(range.end()),
            &replacement,
        );
    }
    result
}

/// Whether the list is inside a `with ...;`, where bare attributes resolve.
pub fn is_with_list(list: &SyntaxNode) -> bool {
    list.ancestors()
        .take_while(|a| a.kind() != SyntaxKind::NODE_ATTRPATH_VALUE)
        .any(|a| a.kind() == SyntaxKind::NODE_WITH)
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

/// Leading whitespace of the line containing `offset`.
fn line_indent(src: &str, offset: usize) -> String {
    let line_start = src[..offset].rfind('\n').map_or(0, |i| i + 1);
    src[line_start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
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
    let multiline = ws.as_ref().is_some_and(|w| w.text().contains('\n'));
    let (indent, closing_indent) = if multiline {
        (binding_indent(set, "    "), last_line(ws.as_ref()))
    } else {
        // An inline set has no indentation of its own, so it follows the line it starts on.
        let base = line_indent(src, usize::from(set.text_range().start()));
        (format!("{}  ", base), base)
    };
    let replacement = format!("\n{}{}\n{}}}", indent, entry, closing_indent);
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
