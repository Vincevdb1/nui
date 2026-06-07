use crate::nix::editor::inputs::remove_input;
use crate::nix::editor::packages::{find_shell_node, get_shell_attr_set};
use crate::nix::parser::{extract_inputs, extract_outputs};
use crate::nix::query::extract_package_attribute_strings;
use color_eyre::Result;
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::path::Path;

pub fn remove_packages(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_names: &[String],
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;
    let mut current_content = content.clone();

    for pkg_name in pkg_names {
        current_content =
            remove_package_from_content(&current_content, system, shell_name, pkg_name)?;

        // Handle input removal if it's no longer used
        if let Some((input_name, _)) = pkg_name.split_once('.') {
            let inputs = extract_inputs(&current_content, None);
            if inputs.iter().any(|i| i.name == input_name) {
                let configs = extract_outputs(&current_content);
                let mut used = false;
                for config in &configs {
                    if let Some(config_content) = &config.content {
                        let attrs = extract_package_attribute_strings(config_content);
                        if attrs
                            .iter()
                            .any(|a| a.starts_with(&format!("{}.", input_name)) || a == input_name)
                        {
                            used = true;
                            break;
                        }
                    }
                }

                if !used {
                    current_content = remove_input(&current_content, input_name)?;
                }
            }
        }
    }

    if current_content != content {
        std::fs::write(flake_path, current_content)?;
    }

    Ok(())
}

pub fn remove_package(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_name: &str,
) -> Result<()> {
    remove_packages(flake_path, system, shell_name, &[pkg_name.to_string()])
}

fn remove_package_from_content(
    content: &str,
    system: &str,
    shell_name: &str,
    pkg_name: &str,
) -> Result<String> {
    let ast = Root::parse(content);
    let root = ast.syntax();

    if let Some(shell_node) = find_shell_node(&root, system, shell_name) {
        if let Some(shell_attr_set) = get_shell_attr_set(&shell_node) {
            return Ok(remove_package_from_shell_attr_set(
                &shell_attr_set,
                pkg_name,
                content,
            ));
        }
    }

    Ok(content.to_string())
}

fn remove_package_from_shell_attr_set(
    set_node: &SyntaxNode,
    pkg_name: &str,
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
        current_content = remove_from_list(&list_node, pkg_name, &current_content);
    }

    current_content
}

fn remove_from_list(list_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut to_remove = Vec::new();
    for child in list_node.children() {
        if (child.kind() == SyntaxKind::NODE_SELECT || child.kind() == SyntaxKind::NODE_IDENT)
            && child.to_string().trim() == pkg_name
        {
            let mut start = child.text_range().start();
            let end = child.text_range().end();

            // Try to include preceding whitespace
            if let Some(prev) = child.prev_sibling_or_token() {
                if prev.kind() == SyntaxKind::TOKEN_WHITESPACE {
                    start = prev.text_range().start();
                }
            }
            to_remove.push((start, end));
        }
    }

    let mut result = content.to_string();
    for (start, end) in to_remove.into_iter().rev() {
        result.replace_range(usize::from(start)..usize::from(end), "");
    }
    result
}
