use crate::nix::model::{Input, Output};
use crate::nix::traits::NixParser;
use color_eyre::Result;
use rnix::{Root, SyntaxKind};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize)]
struct LockFile {
    nodes: HashMap<String, LockNode>,
}

#[derive(Deserialize)]
struct LockNode {
    locked: Option<LockLocked>,
    original: Option<LockOriginal>,
}

#[derive(Deserialize)]
struct LockLocked {
    rev: Option<String>,
}

#[derive(Deserialize)]
struct LockOriginal {
    #[serde(rename = "ref")]
    branch: Option<String>,
}

#[allow(dead_code)]
pub struct FlakeParser;

impl NixParser for FlakeParser {
    type Error = color_eyre::Report;

    fn parse_flake(path: &Path) -> Result<Vec<Input>, Self::Error> {
        let flake_nix_path = path.join("flake.nix");
        let flake_lock_path = path.join("flake.lock");

        let content = std::fs::read_to_string(flake_nix_path)?;
        let lock_content = std::fs::read_to_string(flake_lock_path).ok();

        Ok(extract_inputs(&content, lock_content.as_deref()))
    }

    fn parse_outputs(path: &Path) -> Result<Vec<Output>, Self::Error> {
        fetch_outputs(path)
    }
}

pub fn extract_inputs(content: &str, lock_content: Option<&str>) -> Vec<Input> {
    let mut inputs: HashMap<String, Input> = HashMap::new();

    let lock_data: Option<LockFile> = lock_content.and_then(|c| serde_json::from_str(c).ok());

    if let Ok(collection) = nix_editor::parse::get_collection(content.to_string()) {
        for (key, val) in collection {
            if let Some(rest) = key.strip_prefix("inputs.")
                && let Some(name) = rest.strip_suffix(".url")
            {
                let url = val.trim_matches('"').to_string();
                let (branch, rev) = lock_data
                    .as_ref()
                    .and_then(|lock| {
                        lock.nodes.get(name).map(|node| {
                            let branch = node.original.as_ref().and_then(|o| o.branch.clone());
                            let rev = node.locked.as_ref().and_then(|l| l.rev.clone());
                            (branch, rev)
                        })
                    })
                    .unwrap_or((None, None));

                inputs.insert(
                    name.to_string(),
                    Input {
                        name: name.to_string(),
                        url,
                        branch,
                        rev,
                    },
                );
            }
        }
    }

    inputs.into_values().collect()
}

pub fn normalize_flake_ref(path: &str) -> String {
    if path == "." || path.starts_with('/') || path.starts_with("./") || path.contains(':') {
        path.to_string()
    } else {
        format!("./{}", path)
    }
}

pub fn fetch_outputs(flake_path: &Path) -> Result<Vec<Output>> {
    let mut configs = Vec::new();

    let output = std::process::Command::new("nix")
        .args([
            "flake",
            "show",
            &normalize_flake_ref(flake_path.to_str().unwrap_or(".")),
            "--json",
            "--impure",
        ])
        .output()?;

    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!(
            "Failed to run nix flake show: {}. This might be due to a connection issue or an invalid flake.",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let flake_nix_path = flake_path.join("flake.nix");
    let ast_configs = if let Ok(content) = std::fs::read_to_string(flake_nix_path) {
        extract_outputs(&content)
    } else {
        Vec::new()
    };

    let is_v2 = json.get("version").and_then(|v| v.as_u64()) == Some(2);
    let target_obj = if is_v2 {
        json.get("inventory").and_then(|i| i.as_object())
    } else {
        json.as_object()
    };

    if let Some(obj) = target_obj {
        for (config_type, val) in obj {
            if !matches!(
                config_type.as_str(),
                "nixosConfigurations" | "homeConfigurations" | "devShells" | "darwinConfigurations"
            ) {
                continue;
            }

            let inner_obj_val = if is_v2 {
                val.get("output").and_then(|o| o.get("children"))
            } else {
                Some(val)
            };

            if let Some(inner_obj_val) = inner_obj_val {
                if let Some(inner_obj) = inner_obj_val.as_object() {
                    for (path, inner_val) in inner_obj {
                        if matches!(
                            config_type.as_str(),
                            "devShells" | "packages" | "legacyPackages"
                        ) {
                            let systems_obj_val = if is_v2 {
                                inner_val.get("children")
                            } else {
                                Some(inner_val)
                            };

                            if let Some(systems_obj_val) = systems_obj_val {
                                if let Some(systems_obj) = systems_obj_val.as_object() {
                                    for (name, _) in systems_obj {
                                        let full_path = format!("{}.{}", path, name);
                                        let mut config = Output {
                                            path: full_path.clone(),
                                            name: None,
                                            config_type: config_type.clone(),
                                            content: None,
                                        };

                                        if let Some(ast_match) = ast_configs.iter().find(|c| {
                                            c.config_type == *config_type && c.path == full_path
                                        }) {
                                            config.name = ast_match.name.clone();
                                            config.content = ast_match.content.clone();
                                        }

                                        configs.push(config);
                                    }
                                }
                            }
                        } else {
                            let mut config = Output {
                                path: path.clone(),
                                name: None,
                                config_type: config_type.clone(),
                                content: None,
                            };

                            if let Some(ast_match) = ast_configs
                                .iter()
                                .find(|c| c.config_type == *config_type && c.path == *path)
                            {
                                config.name = ast_match.name.clone();
                                config.content = ast_match.content.clone();
                            }

                            configs.push(config);
                        }
                    }
                }
            }
        }
    }

    configs.sort_by(|a, b| {
        let a_is_default_shell = a.config_type == "devShells" && a.path.ends_with(".default");
        let b_is_default_shell = b.config_type == "devShells" && b.path.ends_with(".default");
        if a_is_default_shell != b_is_default_shell {
            return b_is_default_shell.cmp(&a_is_default_shell);
        }

        let a_is_shell = a.config_type == "devShells";
        let b_is_shell = b.config_type == "devShells";
        if a_is_shell != b_is_shell {
            return b_is_shell.cmp(&a_is_shell);
        }

        a.config_type.cmp(&b.config_type).then(a.path.cmp(&b.path))
    });

    Ok(configs)
}

pub fn extract_outputs(content: &str) -> Vec<Output> {
    let mut configs = Vec::new();

    if let Ok(outputs_val) = nix_editor::read::readvalue(content, "outputs") {
        let mut replacements: HashMap<String, String> = HashMap::new();

        let ast = Root::parse(&outputs_val);
        let mut body_val = outputs_val.clone();

        for node in ast.syntax().descendants() {
            if node.kind() == SyntaxKind::NODE_LAMBDA {
                if let Some(body) = node.children().find(|c| {
                    !matches!(
                        c.kind(),
                        SyntaxKind::NODE_PATTERN
                            | SyntaxKind::TOKEN_COLON
                            | SyntaxKind::TOKEN_WHITESPACE
                            | SyntaxKind::TOKEN_COMMENT
                    )
                }) {
                    body_val = body.to_string();

                    if body.kind() == SyntaxKind::NODE_LET_IN {
                        for child in body.children() {
                            if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                                let mut key = String::new();
                                let mut val = String::new();
                                if let Some(path) = child
                                    .children()
                                    .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
                                {
                                    key = path.text().to_string().trim().to_string();
                                }
                                if let Some(v) = child
                                    .children()
                                    .find(|c| c.kind() != SyntaxKind::NODE_ATTRPATH)
                                {
                                    val = v.text().to_string().trim().trim_matches('"').to_string();
                                }
                                if !key.is_empty() && !val.is_empty() {
                                    replacements.insert(format!("${{{}}}", key), val);
                                }
                            }
                        }

                        if let Some(actual_body) = body.children().last() {
                            body_val = actual_body.to_string();
                        }
                    }
                }
                break;
            }
        }

        if let Ok(collection) = nix_editor::parse::get_collection(body_val) {
            for (key, val) in collection {
                let mut parts = Vec::new();
                let mut current = String::new();
                let mut in_quotes = false;
                for c in key.chars() {
                    if c == '"' {
                        in_quotes = !in_quotes;
                        current.push(c);
                    } else if c == '.' && !in_quotes {
                        parts.push(current.clone());
                        current.clear();
                    } else {
                        current.push(c);
                    }
                }
                parts.push(current);

                if !parts.is_empty() {
                    let config_type = &parts[0];
                    if matches!(
                        config_type.as_str(),
                        "nixosConfigurations"
                            | "homeConfigurations"
                            | "devShells"
                            | "darwinConfigurations"
                            | "packages"
                            | "legacyPackages"
                    ) {
                        let mut path_parts = Vec::new();
                        for part in &parts[1..] {
                            if matches!(
                                part.as_str(),
                                "packages" | "buildInputs" | "nativeBuildInputs"
                            ) {
                                break;
                            }
                            let cleaned_part = part.trim_matches('"');
                            path_parts.push(cleaned_part);
                        }
                        let mut path = path_parts.join(".");

                        for (k, v) in &replacements {
                            path = path.replace(k, v);
                        }

                        if let Ok(inner_collection) = nix_editor::parse::get_collection(val.clone())
                        {
                            for (inner_key, inner_val) in inner_collection {
                                let cleaned_inner_key = inner_key.trim_matches('"').to_string();
                                let mut name_opt = None;
                                if let Ok(name_val) =
                                    nix_editor::read::readvalue(&inner_val, "name")
                                {
                                    name_opt = Some(name_val.trim_matches('"').to_string());
                                }

                                let full_path = if path.is_empty() {
                                    cleaned_inner_key
                                } else {
                                    format!("{}.{}", path, cleaned_inner_key)
                                };

                                configs.push(Output {
                                    path: full_path,
                                    name: name_opt,
                                    config_type: config_type.to_string(),
                                    content: Some(inner_val),
                                });
                            }
                        } else {
                            let mut name_opt = None;
                            if let Ok(name_val) = nix_editor::read::readvalue(&val, "name") {
                                name_opt = Some(name_val.trim_matches('"').to_string());
                            }

                            configs.push(Output {
                                path,
                                name: name_opt,
                                config_type: config_type.to_string(),
                                content: Some(val),
                            });
                        }
                    }
                }
            }
        }
    }

    configs
}
