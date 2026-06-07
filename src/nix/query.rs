use crate::nix::model::{Input, Output, Package};
use crate::nix::parser::{extract_inputs, extract_outputs};
use crate::nix::traits::NixQuery;
use color_eyre::Result;
use rnix::{Root, SyntaxKind};
use std::path::Path;

#[allow(dead_code)]
pub struct FlakeQuery;

impl NixQuery for FlakeQuery {
    type Error = color_eyre::Report;

    fn get_packages(flake_path: &Path, output: &Output) -> Result<Vec<Package>, Self::Error> {
        let content = std::fs::read_to_string(flake_path)?;
        let outputs = extract_outputs(&content);

        let target_output = outputs
            .iter()
            .find(|o| o.path == output.path && o.config_type == output.config_type)
            .ok_or_else(|| color_eyre::eyre::eyre!("Output not found: {}", output.path))?;

        let content = target_output.content.as_deref().unwrap_or("");
        let attr_strings = extract_package_attribute_strings(content);

        Ok(attr_strings
            .into_iter()
            .map(|name| Package {
                name,
                description: String::new(),
                version: None,
                is_unfree: false,
                source_input: None,
            })
            .collect())
    }

    fn get_inputs(flake_path: &Path) -> Result<Vec<Input>, Self::Error> {
        let flake_nix_path = flake_path.join("flake.nix");
        let flake_lock_path = flake_path.join("flake.lock");

        let content = std::fs::read_to_string(flake_nix_path)?;
        let lock_content = std::fs::read_to_string(flake_lock_path).ok();

        Ok(extract_inputs(&content, lock_content.as_deref()))
    }
}

pub fn extract_package_attribute_strings(content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let ast = Root::parse(content);

    for node in ast.syntax().descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                let path_text = attrpath.to_string().trim().to_string();
                if path_text == "packages"
                    || path_text == "buildInputs"
                    || path_text == "nativeBuildInputs"
                {
                    if let Some(val) = node.children().find(|c| {
                        !matches!(
                            c.kind(),
                            SyntaxKind::NODE_ATTRPATH
                                | SyntaxKind::TOKEN_COMMENT
                                | SyntaxKind::TOKEN_WHITESPACE
                        )
                    }) {
                        let list_node = if val.kind() == SyntaxKind::NODE_WITH {
                            val.children().find(|c| c.kind() == SyntaxKind::NODE_LIST)
                        } else if val.kind() == SyntaxKind::NODE_LIST {
                            Some(val)
                        } else {
                            None
                        };

                        if let Some(list) = list_node {
                            for item in list.children() {
                                attrs.push(item.to_string().trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // If no attributes were found, check if the content itself is a list
    if attrs.is_empty() {
        for node in ast.syntax().children() {
            if node.kind() == SyntaxKind::NODE_LIST {
                for item in node.children() {
                    attrs.push(item.to_string().trim().to_string());
                }
            }
        }
    }

    attrs
}
