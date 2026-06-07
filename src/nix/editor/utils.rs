use rnix::{SyntaxKind, SyntaxNode};

pub fn find_path(node: &SyntaxNode, path: &[&str], system: &str) -> Option<SyntaxNode> {
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
                if let Some(attrpath) = child
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

pub fn match_segment(actual: &str, expected: &str, system: &str) -> bool {
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
