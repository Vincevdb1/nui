use crate::nix::editor::packages::{find_shell_node, get_shell_attr_set};
use color_eyre::Result;
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::path::Path;

pub fn add_package(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_name: &str,
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;
    let ast = Root::parse(&content);
    let root = ast.syntax();

    if let Some(shell_node) = find_shell_node(&root, system, shell_name) {
        if let Some(shell_attr_set) = get_shell_attr_set(&shell_node) {
            let new_content = modify_shell_attr_set(&shell_attr_set, pkg_name, &content);
            std::fs::write(flake_path, new_content)?;
            return Ok(());
        }
    }

    // Fallback to nix-editor if AST traversal fails or shell structure is missing
    let query = format!("outputs.devShells.{}.{}.packages", system, shell_name);
    let pkg_val = pkg_name.to_string();

    if let Ok(new_content) = nix_editor::write::addtoarr(&content, &query, vec![pkg_val.clone()]) {
        // Only write if it successfully made a change
        if new_content != content {
            std::fs::write(flake_path, new_content)?;
            return Ok(());
        }
    }

    // Try without "outputs." prefix as well
    let query_no_outputs = format!("devShells.{}.{}.packages", system, shell_name);
    if let Ok(new_content) = nix_editor::write::addtoarr(&content, &query_no_outputs, vec![pkg_val])
    {
        if new_content != content {
            std::fs::write(flake_path, new_content)?;
            return Ok(());
        }
    }

    Err(color_eyre::eyre::eyre!(
        "Could not find or modify devShells packages in flake.nix"
    ))
}

fn modify_shell_attr_set(set_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut packages_node = None;
    for child in set_node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = child
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                let path_string = attrpath.to_string();
                let path_text = path_string.trim();
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
                            packages_node = Some(val);
                            break;
                        } else if val.kind() == SyntaxKind::NODE_WITH {
                            if let Some(list) =
                                val.children().find(|c| c.kind() == SyntaxKind::NODE_LIST)
                            {
                                packages_node = Some(list);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(list_node) = packages_node {
        append_to_list(&list_node, pkg_name, content)
    } else {
        insert_packages_into_set(set_node, pkg_name, content)
    }
}

fn append_to_list(list_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut close_bracket_opt = None;
    for child in list_node.children_with_tokens() {
        if let Some(token) = child.as_token()
            && token.text() == "]"
        {
            close_bracket_opt = Some(token.clone());
        }
    }

    if let Some(close_bracket) = close_bracket_opt {
        let mut item_indent = None;
        let mut is_multiline = false;

        for child in list_node.children_with_tokens() {
            if child.kind() == SyntaxKind::TOKEN_WHITESPACE && child.to_string().contains('\n') {
                is_multiline = true;
                if let Some(last_line) = child.to_string().lines().last() {
                    if item_indent.is_none() {
                        item_indent = Some(last_line.to_string());
                    }
                }
            }
        }

        let item_indent = item_indent.unwrap_or_else(|| "      ".to_string());

        let mut start_of_replacement = close_bracket.text_range().start();
        let mut ws_before_bracket = String::new();
        if let Some(prev) = close_bracket.prev_sibling_or_token()
            && prev.kind() == SyntaxKind::TOKEN_WHITESPACE
        {
            ws_before_bracket = prev.to_string();
            start_of_replacement = prev.text_range().start();
        }

        let new_entry = if is_multiline {
            let closing_bracket_indent = if let Some(last_line) = ws_before_bracket.lines().last() {
                last_line.to_string()
            } else {
                "".to_string()
            };

            let mut final_item_indent = item_indent;
            if final_item_indent == closing_bracket_indent && !final_item_indent.is_empty() {
                final_item_indent.push_str("  ");
            }

            format!(
                "\n{}{}\n{}",
                final_item_indent, pkg_name, closing_bracket_indent
            )
        } else if list_node.children().count() == 0 {
            format!(" {} ", pkg_name)
        } else {
            format!(" {}", pkg_name)
        };

        let mut result = content.to_string();
        result.replace_range(
            usize::from(start_of_replacement)..usize::from(close_bracket.text_range().end()),
            &format!("{}]", new_entry),
        );
        return result;
    }
    content.to_string()
}

fn insert_packages_into_set(set_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut close_brace_opt = None;
    for child in set_node.children_with_tokens() {
        if let Some(token) = child.as_token()
            && token.text() == "}"
        {
            close_brace_opt = Some(token.clone());
        }
    }

    if let Some(close_brace) = close_brace_opt {
        let mut item_indent = "    ".to_string();
        for child in set_node.children() {
            if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                if let Some(prev) = child.prev_sibling_or_token()
                    && prev.kind() == SyntaxKind::TOKEN_WHITESPACE
                {
                    let ws = prev.to_string();
                    if let Some(last_line) = ws.lines().last() {
                        item_indent = last_line.to_string();
                    }
                }
                break;
            }
        }

        let mut ws_before_brace = String::new();
        let mut start_of_replacement = close_brace.text_range().start();
        if let Some(prev) = close_brace.prev_sibling_or_token()
            && prev.kind() == SyntaxKind::TOKEN_WHITESPACE
        {
            ws_before_brace = prev.to_string();
            start_of_replacement = prev.text_range().start();
        }

        let closing_brace_indent = if let Some(last_line) = ws_before_brace.lines().last() {
            last_line.to_string()
        } else {
            "".to_string()
        };

        let new_entry = format!(
            "\n{}packages = [ {} ];\n{}}}",
            item_indent, pkg_name, closing_brace_indent
        );

        let mut result = content.to_string();
        result.replace_range(
            usize::from(start_of_replacement)..usize::from(close_brace.text_range().end()),
            &new_entry,
        );
        return result;
    }
    content.to_string()
}
