use crate::nix::{Configuration, Input};
use color_eyre::Result;
use rnix::{Root, SyntaxKind, SyntaxNode};
use std::collections::HashMap;
use std::path::Path;

pub fn extract_inputs(content: &str) -> Vec<Input> {
    let mut inputs: HashMap<String, Input> = HashMap::new();

    if let Ok(collection) = nix_editor::parse::get_collection(content.to_string()) {
        for (key, val) in collection {
            if let Some(rest) = key.strip_prefix("inputs.")
                && let Some(name) = rest.strip_suffix(".url")
            {
                let url = val.trim_matches('"').to_string();
                inputs.insert(
                    name.to_string(),
                    Input {
                        name: name.to_string(),
                        url,
                    },
                );
            }
        }
    }

    inputs.into_values().collect()
}

pub fn extract_configurations(content: &str) -> Vec<Configuration> {
    let mut configs = Vec::new();

    if let Ok(outputs_val) = nix_editor::read::readvalue(content, "outputs") {
        let mut replacements: HashMap<String, String> = HashMap::new();

        let ast = Root::parse(&outputs_val);
        use rnix::SyntaxKind;
        for node in ast.syntax().descendants() {
            if node.kind() == SyntaxKind::NODE_LET_IN {
                for child in node.children() {
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
            }
        }

        if let Ok(collection) = nix_editor::parse::get_collection(outputs_val) {
            for (key, val) in collection {
                let parts: Vec<&str> = key.split('.').collect();
                if !parts.is_empty() {
                    let config_type = parts[0];
                    if matches!(
                        config_type,
                        "nixosConfigurations"
                            | "homeConfigurations"
                            | "devShells"
                            | "darwinConfigurations"
                    ) {
                        let mut path = parts[1..].join(".");

                        for (k, v) in &replacements {
                            path = path.replace(k, v);
                        }

                        let mut name_opt = None;

                        if let Ok(name_val) = nix_editor::read::readvalue(&val, "name") {
                            name_opt = Some(name_val.trim_matches('"').to_string());
                        }

                        configs.push(Configuration {
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

    // Check if input already exists
    let inputs = extract_inputs(content);
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

    // Add to outputs pattern
    result = add_to_outputs_pattern(&result, &name);

    result
}

fn add_to_outputs_pattern(content: &str, name: &str) -> String {
    let ast = Root::parse(content);
    let root = ast.syntax();
    use rnix::SyntaxKind;

    // Find outputs = ...
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

    // Find the lambda
    let Some(lambda) = outputs_node
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_LAMBDA)
    else {
        return content.to_string();
    };

    // Find the pattern
    let Some(pattern) = lambda
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_PATTERN)
    else {
        return content.to_string();
    };

    // Check if name already exists in pattern
    for child in pattern.children_with_tokens() {
        let text = child.to_string();
        let trimmed = text.trim().trim_matches(',');
        if trimmed == name {
            return content.to_string();
        }
    }

    // Find the closing brace of the pattern
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

    // Determine if it's multiline and what the indentation is
    let mut is_multiline = false;
    for child in pattern.children_with_tokens() {
        if child.kind() == SyntaxKind::TOKEN_WHITESPACE && child.to_string().contains('\n') {
            is_multiline = true;
        }
    }

    if is_multiline {
        // Find if there's a comma before the closing brace
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
            // Check if there are any entries at all
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
        // Single line
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_add_package() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let flake_path = temp_dir.join("test_add_package_flake.nix");
        let content = r#"{
  outputs = { self, nixpkgs }: {
    devShells.x86_64-linux.default = {
      packages = [ ];
    };
  };
}"#;
        fs::write(&flake_path, content)?;

        add_package(&flake_path, "x86_64-linux", "default", "hello")?;

        let updated_content = fs::read_to_string(&flake_path)?;
        assert!(updated_content.contains("hello"));
        assert!(!updated_content.contains("\"hello\""));
        assert!(updated_content.contains("packages"));

        fs::remove_file(flake_path)?;
        Ok(())
    }

    #[test]
    fn test_add_package_prefixed() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let flake_path = temp_dir.join("test_add_package_prefixed_flake.nix");
        let content = r#"{
  outputs = { self, nixpkgs, nixpkgs-abc }: {
    devShells.x86_64-linux.default = {
      packages = [ ];
    };
  };
}"#;
        fs::write(&flake_path, content)?;

        add_package(&flake_path, "x86_64-linux", "default", "nixpkgs-abc.hello")?;

        let updated_content = fs::read_to_string(&flake_path)?;
        assert!(updated_content.contains("nixpkgs-abc.hello"));
        assert!(!updated_content.contains("\"nixpkgs-abc.hello\""));

        fs::remove_file(flake_path)?;
        Ok(())
    }

    #[test]
    fn test_add_to_outputs_pattern_multiline_no_comma() {
        let content = r#"{
  outputs =
    {
      self,
      nixpkgs
    }:
    { };
}"#;
        let result = add_to_outputs_pattern(content, "nixpkgs-abc");
        println!("Result: '{}'", result);
        assert!(result.contains("nixpkgs,"));
        assert!(result.contains("nixpkgs-abc,"));
    }

    #[test]
    fn test_add_to_outputs_pattern_multiline() {
        let content = r#"{
  outputs =
    {
      self,
      nixpkgs,
    }:
    { };
}"#;
        let result = add_to_outputs_pattern(content, "nixpkgs-abc");
        println!("Result: '{}'", result);
        assert!(result.contains("nixpkgs-abc,"));
        assert!(result.contains("nixpkgs,"));
    }

    #[test]
    fn test_add_to_outputs_pattern_single_line() {
        let content = r#"{
  outputs = { self, nixpkgs }: { };
}"#;
        let result = add_to_outputs_pattern(content, "nixpkgs-abc");
        println!("Result: '{}'", result);
        assert!(result.contains("{ self, nixpkgs, nixpkgs-abc }"));
    }

    #[test]
    fn test_add_nixpkgs_input() {
        let content = r#"{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs }: { };
}"#;
        let hash = "abc123def";
        let result = add_nixpkgs_input(content, hash);
        
        assert!(result.contains("nixpkgs-abc123def.url"));
        assert!(result.contains("github:nixos/nixpkgs/abc123def"));
        assert!(result.contains("nixpkgs-abc123def")); // in outputs
    }

    #[test]
    fn test_version_pinning_flow() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let flake_path = temp_dir.join("test_pinning_flow_flake.nix");
        let content = r#"{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs }: {
    devShells.x86_64-linux.default = {
      packages = [ ];
    };
  };
}"#;
        fs::write(&flake_path, content)?;

        let hash = "abc123def";
        let pkg_name = "hello";
        let prefixed_pkg = format!("nixpkgs-{}.{}", hash, pkg_name);

        // 1. Add nixpkgs input
        let content = fs::read_to_string(&flake_path)?;
        let new_content = add_nixpkgs_input(&content, hash);
        fs::write(&flake_path, new_content)?;

        // 2. Add package
        add_package(&flake_path, "x86_64-linux", "default", &prefixed_pkg)?;

        let final_content = fs::read_to_string(&flake_path)?;
        
        // Check input
        assert!(final_content.contains("nixpkgs-abc123def.url"));
        assert!(final_content.contains("github:nixos/nixpkgs/abc123def"));
        
        // Check outputs pattern
        assert!(final_content.contains("nixpkgs-abc123def"));
        
        // Check package
        assert!(final_content.contains("nixpkgs-abc123def.hello"));

        fs::remove_file(flake_path)?;
        Ok(())
    }
}
