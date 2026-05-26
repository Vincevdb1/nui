use crate::nix::{Input, Output};
use color_eyre::Result;
use rnix::{Root, SyntaxKind, SyntaxNode};
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

pub fn extract_inputs(content: &str, lock_content: Option<&str>) -> Vec<Input> {
    let mut inputs: HashMap<String, Input> = HashMap::new();

    let lock_data: Option<LockFile> = lock_content
        .and_then(|c| serde_json::from_str(c).ok());

    if let Ok(collection) = nix_editor::parse::get_collection(content.to_string()) {
        for (key, val) in collection {
            if let Some(rest) = key.strip_prefix("inputs.")
                && let Some(name) = rest.strip_suffix(".url")
            {
                let url = val.trim_matches('"').to_string();
                let (branch, rev) = lock_data.as_ref().and_then(|lock| {
                    lock.nodes.get(name).map(|node| {
                        let branch = node.original.as_ref().and_then(|o| o.branch.clone());
                        let rev = node.locked.as_ref().and_then(|l| l.rev.clone());
                        (branch, rev)
                    })
                }).unwrap_or((None, None));

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
                "nixosConfigurations"
                    | "homeConfigurations"
                    | "devShells"
                    | "darwinConfigurations"
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
                        if matches!(config_type.as_str(), "devShells" | "packages" | "legacyPackages") {
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
        use rnix::SyntaxKind;
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
                            if matches!(part.as_str(), "packages" | "buildInputs" | "nativeBuildInputs") {
                                break;
                            }
                            let cleaned_part = part.trim_matches('"');
                            path_parts.push(cleaned_part);
                        }
                        let mut path = path_parts.join(".");

                        for (k, v) in &replacements {
                            path = path.replace(k, v);
                        }

                        if let Ok(inner_collection) = nix_editor::parse::get_collection(val.clone()) {
                            for (inner_key, inner_val) in inner_collection {
                                let cleaned_inner_key = inner_key.trim_matches('"').to_string();
                                let mut name_opt = None;
                                if let Ok(name_val) = nix_editor::read::readvalue(&inner_val, "name") {
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

#[allow(dead_code)]
pub fn add_nixpkgs_input(content: &str, hash: &str) -> String {
    let name = format!("nixpkgs-{}", hash);
    let url = format!("github:nixos/nixpkgs/{}", hash);
    add_input(content, &name, &url)
}

#[allow(dead_code)]
pub fn add_input(content: &str, name: &str, url: &str) -> String {
    let name = name.replace('.', "-");

    let inputs = extract_inputs(content, None);
    let already_has_input = inputs.iter().any(|i| i.name == name);

    let mut result = if !already_has_input {
        let ast = Root::parse(content);
        use rnix::SyntaxKind;

        let mut updated = None;
        for node in ast.syntax().descendants() {
            if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                let has_inputs_path = node.children().any(|c| {
                    c.kind() == SyntaxKind::NODE_ATTRPATH && c.text().to_string().trim() == "inputs"
                });

                if has_inputs_path
                    && let Some(set_node) = node
                        .children()
                        .find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET)
                {
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

                        let closing_brace_indent =
                            if let Some(last_line) = ws_before_brace.lines().last() {
                                last_line.to_string()
                            } else {
                                "".to_string()
                            };

                        let new_entry = format!(
                            "\n{}{}.url = \"{}\";\n{}}}",
                            item_indent, name, url, closing_brace_indent
                        );

                        let mut res = content.to_string();
                        let start: usize = start_of_replacement.into();
                        let end: usize = close_brace.text_range().end().into();
                        res.replace_range(start..end, &new_entry);
                        updated = Some(res);
                        break;
                    }
                }
            }
        }

        updated.unwrap_or_else(|| {
            let query = format!("inputs.{}.url", name);
            let value = format!("\"{}\"", url);
            match nix_editor::write::write(content, &query, &value) {
                Ok(new_content) => new_content,
                Err(_) => content.to_string(),
            }
        })
    } else {
        content.to_string()
    };

    result = add_to_outputs_pattern(&result, &name);

    result
}

fn add_to_outputs_pattern(content: &str, name: &str) -> String {
    let ast = Root::parse(content);
    let root = ast.syntax();
    use rnix::SyntaxKind;

    let mut outputs_node = None;
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
                if attrpath.text().to_string().trim() == "outputs" {
                    outputs_node = Some(node);
                    break;
                }
            }
        }
    }

    let Some(outputs_node) = outputs_node else {
        return content.to_string();
    };

    let Some(lambda) = outputs_node
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_LAMBDA)
    else {
        return content.to_string();
    };

    let Some(pattern) = lambda
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_PATTERN)
    else {
        return content.to_string();
    };

    for child in pattern.children_with_tokens() {
        let text = child.to_string();
        let trimmed = text.trim().trim_matches(',');
        if trimmed == name {
            return content.to_string();
        }
    }

    let mut close_brace = None;
    for child in pattern.children_with_tokens() {
        if let Some(token) = child.as_token() {
            if token.text() == "}" {
                close_brace = Some(token.clone());
            }
        }
    }

    let Some(close_brace) = close_brace else {
        return content.to_string();
    };

    let mut result = content.to_string();
    let mut start_of_replacement = close_brace.text_range().start();
    let mut ws_before_brace = String::new();
    if let Some(prev) = close_brace.prev_sibling_or_token() {
        if prev.kind() == SyntaxKind::TOKEN_WHITESPACE {
            ws_before_brace = prev.to_string();
            if !ws_before_brace.contains('\n') {
                start_of_replacement = prev.text_range().start();
            }
        }
    }

    let mut is_multiline = false;
    for child in pattern.children_with_tokens() {
        if child.kind() == SyntaxKind::TOKEN_WHITESPACE && child.to_string().contains('\n') {
            is_multiline = true;
        }
    }

    if is_multiline {
        let mut has_comma = false;
        if let Some(prev) = close_brace.prev_sibling_or_token() {
            let mut curr = Some(prev);
            while let Some(c) = curr {
                if c.kind() == SyntaxKind::TOKEN_COMMA {
                    has_comma = true;
                    break;
                }
                if !matches!(
                    c.kind(),
                    SyntaxKind::TOKEN_WHITESPACE | SyntaxKind::TOKEN_COMMENT
                ) {
                    break;
                }
                curr = c.prev_sibling_or_token();
            }
        }

        let mut entry_indent = "      ".to_string();
        for child in pattern.children_with_tokens() {
            if child.kind() == SyntaxKind::NODE_PAT_ENTRY || child.kind() == SyntaxKind::TOKEN_IDENT
            {
                if let Some(prev) = child.prev_sibling_or_token() {
                    if prev.kind() == SyntaxKind::TOKEN_WHITESPACE {
                        if let Some(last_line) = prev.to_string().lines().last() {
                            entry_indent = last_line.to_string();
                        }
                    }
                }
                break;
            }
        }

        let closing_brace_indent = if let Some(last_line) = ws_before_brace.lines().last() {
            last_line.to_string()
        } else {
            "".to_string()
        };

        let insertion = if has_comma {
            format!("{}{},\n{}", entry_indent, name, closing_brace_indent)
        } else {
            let has_entries = pattern.children().any(|c| {
                matches!(
                    c.kind(),
                    SyntaxKind::NODE_PAT_ENTRY | SyntaxKind::TOKEN_IDENT
                )
            });
            if has_entries {
                format!(",\n{}{},\n{}", entry_indent, name, closing_brace_indent)
            } else {
                format!("\n{}{},\n{}", entry_indent, name, closing_brace_indent)
            }
        };

        let start: usize = close_brace.text_range().start().into();
        if let Some(prev) = close_brace.prev_sibling_or_token()
            && prev.kind() == SyntaxKind::TOKEN_WHITESPACE
        {
            let start: usize = prev.text_range().start().into();
            result.replace_range(start..close_brace.text_range().start().into(), &insertion);
        } else {
            result.insert_str(start, &insertion);
        }
    } else {
        let has_entries = pattern.children().any(|c| {
            matches!(
                c.kind(),
                SyntaxKind::NODE_PAT_ENTRY | SyntaxKind::TOKEN_IDENT
            )
        });
        let insertion = if has_entries {
            format!(", {} ", name)
        } else {
            format!(" {} ", name)
        };
        let start: usize = start_of_replacement.into();
        let end: usize = close_brace.text_range().start().into();
        result.replace_range(start..end, &insertion);
    }

    result
}

pub fn extract_package_attribute_strings(content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let ast = Root::parse(content);
    
    for node in ast.syntax().descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
                let path_text = attrpath.to_string().trim().to_string();
                if path_text == "packages" || path_text == "buildInputs" || path_text == "nativeBuildInputs" {
                    if let Some(val) = node.children().find(|c| !matches!(c.kind(), SyntaxKind::NODE_ATTRPATH | SyntaxKind::TOKEN_COMMENT | SyntaxKind::TOKEN_WHITESPACE)) {
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

pub fn add_package(flake_path: &Path, system: &str, shell_name: &str, pkg_name: &str) -> Result<()> {
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
    if let Ok(new_content) = nix_editor::write::addtoarr(&content, &query_no_outputs, vec![pkg_val]) {
        if new_content != content {
            std::fs::write(flake_path, new_content)?;
            return Ok(());
        }
    }

    Err(color_eyre::eyre::eyre!(
        "Could not find or modify devShells packages in flake.nix"
    ))
}

fn find_shell_node(root: &SyntaxNode, system: &str, shell_name: &str) -> Option<SyntaxNode> {
    // Search for devShells attribute anywhere in the AST
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
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
                         if match_segment(&segments[1], system, system) && segments[2] == shell_name {
                             return Some(val);
                         }
                    } else if segments.len() == 1 {
                        // Recurse into the value set
                        if let Some(res) = find_path(&val, &[system, shell_name], system) {
                            return Some(res);
                        }
                    } else if segments.len() == 2 {
                        if match_segment(&segments[1], system, system) {
                            if let Some(res) = find_path(&val, &[shell_name], system) {
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

fn find_path(node: &SyntaxNode, path: &[&str], system: &str) -> Option<SyntaxNode> {
    if path.is_empty() {
        return Some(node.clone());
    }

    let target = path[0];

    if node.kind() == SyntaxKind::NODE_LAMBDA {
        if let Some(body) = node.children().find(|c| {
            matches!(
                c.kind(),
                SyntaxKind::NODE_ATTR_SET | SyntaxKind::NODE_LET_IN | SyntaxKind::NODE_WITH
            )
        }) {
            return find_path(&body, path, system);
        }
    }

    if node.kind() == SyntaxKind::NODE_ATTR_SET {
        for child in node.children() {
            if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                if let Some(attrpath) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
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

                    if match_segment(&segments[0], target, system) {
                        let mut i = 0;
                        while i < segments.len()
                            && i < path.len()
                            && match_segment(&segments[i], path[i], system)
                        {
                            i += 1;
                        }

                        if i == segments.len() {
                            let val = child.children().find(|c| {
                                !matches!(
                                    c.kind(),
                                    SyntaxKind::NODE_ATTRPATH
                                        | SyntaxKind::TOKEN_COMMENT
                                        | SyntaxKind::TOKEN_WHITESPACE
                                )
                            })?;
                            return find_path(&val, &path[i..], system);
                        }
                    }
                }
            }
        }
    }

    if node.kind() == SyntaxKind::NODE_LET_IN || node.kind() == SyntaxKind::NODE_WITH {
        if let Some(body) = node.children().find(|c| {
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
        }) {
            return find_path(&body, path, system);
        }
    }

    if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
        if let Some(val) = node.children().find(|c| {
            !matches!(
                c.kind(),
                SyntaxKind::NODE_ATTRPATH
                    | SyntaxKind::TOKEN_COMMENT
                    | SyntaxKind::TOKEN_WHITESPACE
            )
        }) {
            return find_path(&val, path, system);
        }
    }

    None
}

fn match_segment(actual: &str, expected: &str, system: &str) -> bool {
    let actual = actual.trim_matches('"');
    if actual == expected {
        return true;
    }

    // Handle ${system} and just system if it matches the expected system
    if expected == system && (actual == "${system}" || actual == "system") {
        return true;
    }

    // Handle interpolated system like "${system}" or "x86_64-linux"
    if actual.starts_with("${") && actual.ends_with('}') {
        let inner = actual[2..actual.len() - 1].trim();
        if inner == "system" && expected == system {
            return true;
        }
    }

    false
}

fn get_shell_attr_set(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        SyntaxKind::NODE_APPLY => node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET),
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

fn modify_shell_attr_set(set_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut packages_node = None;
    for child in set_node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
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

pub fn remove_packages(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_names: &[String],
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;
    let mut current_content = content.clone();

    for pkg_name in pkg_names {
        current_content = remove_package_from_content(&current_content, system, shell_name, pkg_name)?;

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

#[allow(dead_code)]
pub fn replace_package(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    old_pkg: &str,
    new_pkg: &str,
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;
    let ast = Root::parse(&content);
    let root = ast.syntax();

    if let Some(shell_node) = find_shell_node(&root, system, shell_name) {
        if let Some(shell_attr_set) = get_shell_attr_set(&shell_node) {
            let new_content =
                replace_package_in_shell_attr_set(&shell_attr_set, old_pkg, new_pkg, &content);
            if new_content != content {
                std::fs::write(flake_path, new_content)?;
            }
            return Ok(());
        }
    }

    Err(color_eyre::eyre::eyre!(
        "Could not find or modify devShells packages in flake.nix"
    ))
}

fn replace_package_in_shell_attr_set(
    set_node: &SyntaxNode,
    old_pkg: &str,
    new_pkg: &str,
    content: &str,
) -> String {
    let mut packages_nodes = Vec::new();
    for child in set_node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
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

#[allow(dead_code)]
fn replace_in_list(list_node: &SyntaxNode, old_pkg: &str, new_pkg: &str, content: &str) -> String {
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

pub fn remove_package(flake_path: &Path, system: &str, shell_name: &str, pkg_name: &str) -> Result<()> {
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

fn remove_package_from_shell_attr_set(set_node: &SyntaxNode, pkg_name: &str, content: &str) -> String {
    let mut packages_nodes = Vec::new();
    for child in set_node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
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

pub fn remove_input(content: &str, input_name: &str) -> Result<String> {
    let mut new_content = content.to_string();

    let ast = Root::parse(content);
    let root = ast.syntax();

    // Find inputs = { ... }
    let mut inputs_node = None;
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
                if attrpath.text().to_string().trim() == "inputs" {
                    if let Some(val) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET)
                    {
                        inputs_node = Some(val);
                        break;
                    }
                }
            }
        }
    }

    if let Some(inputs_set) = inputs_node {
        for child in inputs_set.children() {
            if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                if let Some(attrpath) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
                {
                    let path_text = attrpath.to_string().trim().to_string();
                    if path_text == input_name || path_text.starts_with(&format!("{}.", input_name))
                    {
                        let mut start = child.text_range().start();
                        let mut end = child.text_range().end();

                        // Include semicolon
                        let mut next = child.next_sibling_or_token();
                        while let Some(n) = next {
                            if n.kind() == SyntaxKind::TOKEN_SEMICOLON {
                                end = n.text_range().end();
                                break;
                            }
                            if !matches!(
                                n.kind(),
                                SyntaxKind::TOKEN_WHITESPACE | SyntaxKind::TOKEN_COMMENT
                            ) {
                                break;
                            }
                            next = n.next_sibling_or_token();
                        }

                        // Include preceding whitespace
                        if let Some(prev) = child.prev_sibling_or_token() {
                            if prev.kind() == SyntaxKind::TOKEN_WHITESPACE {
                                start = prev.text_range().start();
                            }
                        }

                        new_content.replace_range(usize::from(start)..usize::from(end), "");
                        // Re-parse to handle multiple attributes for the same input
                        return remove_input(&new_content, input_name);
                    }
                }
            }
        }
    }

    new_content = remove_from_outputs_pattern(&new_content, input_name);

    Ok(new_content)
}

fn remove_from_outputs_pattern(content: &str, name: &str) -> String {
    let ast = Root::parse(content);
    let root = ast.syntax();
    use rnix::SyntaxKind;

    let mut outputs_node = None;
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
                if attrpath.text().to_string().trim() == "outputs" {
                    outputs_node = Some(node);
                    break;
                }
            }
        }
    }

    let Some(outputs_node) = outputs_node else {
        return content.to_string();
    };

    let Some(lambda) = outputs_node
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_LAMBDA)
    else {
        return content.to_string();
    };

    let Some(pattern) = lambda
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_PATTERN)
    else {
        return content.to_string();
    };

    for child in pattern.children_with_tokens() {
        let text = child.to_string();
        let trimmed = text.trim().trim_matches(',');
        if trimmed == name {
            let mut start = child.text_range().start();
            let mut end = child.text_range().end();

            let mut next = child.next_sibling_or_token();
            while let Some(n) = next {
                if n.kind() == SyntaxKind::TOKEN_COMMA {
                    end = n.text_range().end();
                    break;
                }
                if !matches!(
                    n.kind(),
                    SyntaxKind::TOKEN_WHITESPACE | SyntaxKind::TOKEN_COMMENT
                ) {
                    break;
                }
                next = n.next_sibling_or_token();
            }

            if end == child.text_range().end() {
                let mut prev = child.prev_sibling_or_token();
                while let Some(p) = prev {
                    if p.kind() == SyntaxKind::TOKEN_COMMA {
                        start = p.text_range().start();
                        break;
                    }
                    if !matches!(
                        p.kind(),
                        SyntaxKind::TOKEN_WHITESPACE | SyntaxKind::TOKEN_COMMENT
                    ) {
                        break;
                    }
                    prev = p.prev_sibling_or_token();
                }
            }

            let mut result = content.to_string();
            result.replace_range(usize::from(start)..usize::from(end), "");
            return result;
        }
    }
    content.to_string()
}


