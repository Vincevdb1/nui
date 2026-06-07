use color_eyre::Result;
use rnix::{Root, SyntaxKind};

pub fn remove_input(content: &str, input_name: &str) -> Result<String> {
    let mut new_content = content.to_string();

    let ast = Root::parse(content);
    let root = ast.syntax();

    // Find inputs = { ... }
    let mut inputs_node = None;
    for node in root.descendants() {
        if node.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath) = node
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                if attrpath.text().to_string().trim() == "inputs" {
                    if let Some(val) = node
                        .children()
                        .find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET)
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
                if let Some(attrpath) = child
                    .children()
                    .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
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
