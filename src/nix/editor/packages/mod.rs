pub mod add;
pub mod pin;
pub mod remove;

pub use add::add_package;
pub use pin::{pin_package, unpin_package};
pub use remove::{remove_package, remove_packages};

use crate::nix::editor::utils::match_segment;
use rnix::{SyntaxKind, SyntaxNode};

pub(crate) fn find_shell_node(
    root: &SyntaxNode,
    system: &str,
    shell_name: &str,
) -> Option<SyntaxNode> {
    // Search for devShells attribute anywhere in the AST
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                let segments: Vec<String> = attrpath
                    .children_with_tokens()
                    .filter(|c| {
                        !matches!(
                            c.kind(),
                            SyntaxKind::TOKEN_DOT | SyntaxKind::TOKEN_WHITESPACE
                        )
                    })
                    .map(|c| c.to_string().trim().to_string())
                    .collect();

                if segments.is_empty() {
                    continue;
                }

                if segments[0] == "devShells" {
                    // This node's value is what we want to search in
                    let val = node.children().find(|c| {
                        !matches!(
                            c.kind(),
                            SyntaxKind::NODE_ATTRPATH
                                | SyntaxKind::TOKEN_COMMENT
                                | SyntaxKind::TOKEN_WHITESPACE
                        )
                    })?;

                    if segments.len() >= 3 {
                        if match_segment(&segments[1], system, system) && segments[2] == shell_name
                        {
                            return Some(val);
                        }
                    } else if segments.len() == 1 {
                        // Recurse into the value set
                        if let Some(res) = crate::nix::editor::utils::find_path(
                            &val,
                            &[system, shell_name],
                            system,
                        ) {
                            return Some(res);
                        }
                    } else if segments.len() == 2 {
                        if match_segment(&segments[1], system, system) {
                            if let Some(res) =
                                crate::nix::editor::utils::find_path(&val, &[shell_name], system)
                            {
                                return Some(res);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn get_shell_attr_set(node: &SyntaxNode) -> Option<SyntaxNode> {
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
            get_shell_attr_set(&body)
        }
        _ => None,
    }
}

pub(crate) fn replace_package_in_shell_attr_set(
    set_node: &SyntaxNode,
    old_pkg: &str,
    new_pkg: &str,
    content: &str,
) -> String {
    let mut packages_nodes = Vec::new();
    for child in set_node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = child
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                let path_text = attrpath.to_string().trim().to_string();
                if path_text == "packages"
                    || path_text == "buildInputs"
                    || path_text == "nativeBuildInputs"
                {
                    if let Some(val) = child.children().find(|c| {
                        !matches!(
                            c.kind(),
                            SyntaxKind::NODE_ATTRPATH
                                | SyntaxKind::TOKEN_COMMENT
                                | SyntaxKind::TOKEN_WHITESPACE
                        )
                    }) {
                        if val.kind() == SyntaxKind::NODE_LIST {
                            packages_nodes.push(val);
                        } else if val.kind() == SyntaxKind::NODE_WITH {
                            if let Some(list) =
                                val.children().find(|c| c.kind() == SyntaxKind::NODE_LIST)
                            {
                                packages_nodes.push(list);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut current_content = content.to_string();
    for list_node in packages_nodes {
        current_content = replace_in_list(&list_node, old_pkg, new_pkg, &current_content);
    }

    current_content
}

pub(crate) fn replace_in_list(
    list_node: &SyntaxNode,
    old_pkg: &str,
    new_pkg: &str,
    content: &str,
) -> String {
    let mut to_replace = Vec::new();
    for child in list_node.children() {
        if (child.kind() == SyntaxKind::NODE_SELECT || child.kind() == SyntaxKind::NODE_IDENT)
            && child.to_string().trim() == old_pkg
        {
            to_replace.push((child.text_range().start(), child.text_range().end()));
        }
    }

    let mut result = content.to_string();
    for (start, end) in to_replace.into_iter().rev() {
        result.replace_range(usize::from(start)..usize::from(end), new_pkg);
    }
    result
}
