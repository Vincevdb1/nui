use super::ast::{
    append_to_set, inputs_set, last_line, last_token, outputs_pattern, whitespace_before,
};
use crate::flake::parse;
use rnix::{NodeOrToken, Root, SyntaxElement, SyntaxKind};

/// Declares the input (dots become dashes) and adds it to the outputs pattern.
pub fn add_input(src: &str, name: &str, url: &str) -> String {
    let name = name.replace('.', "-");
    let declared = parse::inputs(src, None).iter().any(|i| i.name == name);

    let result = if declared {
        src.to_string()
    } else {
        let root = Root::parse(src).syntax();
        inputs_set(&root)
            .and_then(|set| append_to_set(&set, &format!("{}.url = \"{}\";", name, url), src))
            .unwrap_or_else(|| {
                nix_editor::write::write(
                    src,
                    &format!("inputs.{}.url", name),
                    &format!("\"{}\"", url),
                )
                .unwrap_or_else(|_| src.to_string())
            })
    };
    add_to_outputs_pattern(&result, &name)
}

/// Removes every `name.*` attribute from `inputs` and drops it from the outputs pattern.
pub fn remove_input(src: &str, name: &str) -> String {
    let mut result = src.to_string();
    loop {
        let root = Root::parse(&result).syntax();
        let Some(binding) = inputs_set(&root).and_then(|set| {
            set.children()
                .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
                .find(|c| {
                    parse::binding_path(c)
                        .is_some_and(|p| p == name || p.starts_with(&format!("{}.", name)))
                })
        }) else {
            break;
        };

        let mut start = binding.text_range().start();
        let mut end = binding.text_range().end();
        if let Some(semicolon) = next_significant(binding.next_sibling_or_token())
            .filter(|t| t.kind() == SyntaxKind::TOKEN_SEMICOLON)
        {
            end = semicolon.text_range().end();
        }
        if let Some(prev) = binding
            .prev_sibling_or_token()
            .filter(|p| p.kind() == SyntaxKind::TOKEN_WHITESPACE)
        {
            start = prev.text_range().start();
        }
        result.replace_range(usize::from(start)..usize::from(end), "");
    }
    remove_from_outputs_pattern(&result, name)
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TOKEN_WHITESPACE | SyntaxKind::TOKEN_COMMENT
    )
}

fn next_significant(mut el: Option<SyntaxElement>) -> Option<SyntaxElement> {
    while let Some(e) = el {
        if !is_trivia(e.kind()) {
            return Some(e);
        }
        el = e.next_sibling_or_token();
    }
    None
}

fn prev_significant(mut el: Option<SyntaxElement>) -> Option<SyntaxElement> {
    while let Some(e) = el {
        if !is_trivia(e.kind()) {
            return Some(e);
        }
        el = e.prev_sibling_or_token();
    }
    None
}

fn is_entry(el: &SyntaxElement) -> bool {
    matches!(
        el.kind(),
        SyntaxKind::NODE_PAT_ENTRY | SyntaxKind::TOKEN_IDENT
    )
}

fn add_to_outputs_pattern(src: &str, name: &str) -> String {
    let root = Root::parse(src).syntax();
    let Some(pattern) = outputs_pattern(&root) else {
        return src.to_string();
    };
    if pattern
        .children_with_tokens()
        .any(|c| c.to_string().trim().trim_matches(',') == name)
    {
        return src.to_string();
    }
    let Some(close) = last_token(&pattern, "}") else {
        return src.to_string();
    };

    // Nothing may follow `...`, so the entry goes in front of it.
    if let Some(ellipsis) = pattern
        .children_with_tokens()
        .filter_map(|c| c.into_token())
        .find(|t| t.kind() == SyntaxKind::TOKEN_ELLIPSIS)
    {
        let separator =
            whitespace_before(&ellipsis).map_or(" ".to_string(), |w| w.text().to_string());
        let mut result = src.to_string();
        result.insert_str(
            usize::from(ellipsis.text_range().start()),
            &format!("{},{}", name, separator),
        );
        return result;
    }

    let ws = whitespace_before(&close);
    let has_entries = pattern.children_with_tokens().any(|c| is_entry(&c));
    let is_multiline = pattern
        .children_with_tokens()
        .any(|c| c.kind() == SyntaxKind::TOKEN_WHITESPACE && c.to_string().contains('\n'));

    let close_start = usize::from(close.text_range().start());
    let mut result = src.to_string();

    if is_multiline {
        let trailing_comma = prev_significant(close.prev_sibling_or_token())
            .is_some_and(|e| e.kind() == SyntaxKind::TOKEN_COMMA);
        let entry_indent = pattern
            .children_with_tokens()
            .find(is_entry)
            .and_then(|entry| entry.prev_sibling_or_token())
            .filter(|p| p.kind() == SyntaxKind::TOKEN_WHITESPACE)
            .and_then(|p| p.to_string().lines().last().map(str::to_string))
            .unwrap_or_else(|| "      ".to_string());
        let closing_indent = last_line(ws.as_ref());

        let insertion = match (trailing_comma, has_entries) {
            (true, _) => format!("\n{}{},\n{}", entry_indent, name, closing_indent),
            (false, true) => format!(",\n{}{},\n{}", entry_indent, name, closing_indent),
            (false, false) => format!("\n{}{},\n{}", entry_indent, name, closing_indent),
        };
        let start = ws
            .map(|w| usize::from(w.text_range().start()))
            .unwrap_or(close_start);
        result.replace_range(start..close_start, &insertion);
    } else {
        let insertion = if has_entries {
            format!(", {} ", name)
        } else {
            format!(" {} ", name)
        };
        let start = ws
            .filter(|w| !w.text().contains('\n'))
            .map(|w| usize::from(w.text_range().start()))
            .unwrap_or(close_start);
        result.replace_range(start..close_start, &insertion);
    }
    result
}

fn remove_from_outputs_pattern(src: &str, name: &str) -> String {
    let root = Root::parse(src).syntax();
    let Some(pattern) = outputs_pattern(&root) else {
        return src.to_string();
    };
    let Some(entry) = pattern
        .children_with_tokens()
        .find(|c| c.to_string().trim().trim_matches(',') == name)
    else {
        return src.to_string();
    };

    let mut start = entry.text_range().start();
    let mut end = entry.text_range().end();
    match next_significant(entry.next_sibling_or_token()) {
        Some(NodeOrToken::Token(t)) if t.kind() == SyntaxKind::TOKEN_COMMA => {
            end = t.text_range().end();
            // Take the separating whitespace along, so no gap is left behind.
            let before = entry
                .prev_sibling_or_token()
                .filter(|p| p.kind() == SyntaxKind::TOKEN_WHITESPACE);
            let after = t
                .next_sibling_or_token()
                .filter(|n| n.kind() == SyntaxKind::TOKEN_WHITESPACE)
                .filter(|n| !n.to_string().contains('\n'));
            if let Some(before) = before {
                start = before.text_range().start();
            } else if let Some(after) = after {
                end = after.text_range().end();
            }
        }
        _ => {
            if let Some(comma) = prev_significant(entry.prev_sibling_or_token())
                .filter(|e| e.kind() == SyntaxKind::TOKEN_COMMA)
            {
                start = comma.text_range().start();
            }
        }
    }

    let mut result = src.to_string();
    result.replace_range(usize::from(start)..usize::from(end), "");
    result
}
