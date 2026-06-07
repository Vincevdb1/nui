use crate::nix::parser::extract_inputs;
use rnix::{Root, SyntaxKind};

pub fn add_input(content: &str, name: &str, url: &str) -> String {
    let name = name.replace('.', "-");

    let inputs = extract_inputs(content, None);
    let already_has_input = inputs.iter().any(|i| i.name == name);

    let mut result = if !already_has_input {
        let ast = Root::parse(content);

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

    let mut outputs_node = None;
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
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
