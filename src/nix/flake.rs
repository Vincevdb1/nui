use crate::nix::Input;
use rnix::{
    Root,
    ast::{AttrpathValue, Expr, HasEntry, Entry},
};
use rowan::ast::AstNode;
use std::collections::HashMap;

pub fn extract_inputs(content: &str) -> Vec<Input> {
    let root = Root::parse(content).tree();

    let attr_set = match root.expr() {
        Some(Expr::AttrSet(s)) => s,
        _ => return vec![],
    };

    let inputs_av = match attr_set
        .entries()
        .filter_map(as_attrpath_value)
        .find(|av| is_key(av, "inputs"))
    {
        Some(av) => av,
        _ => return vec![],
    };

    let inputs_set = match inputs_av.value() {
        Some(Expr::AttrSet(s)) => s,
        _ => return vec![],
    };

    let mut inputs: HashMap<String, Input> = HashMap::new();

    for av in inputs_set.entries().filter_map(as_attrpath_value) {
        let mut parts = match av.attrpath() {
            Some(p) => p.attrs().map(|a| a.to_string().trim().to_string()),
            None => continue,
        };

        let Some(name) = parts.next() else {
            continue;
        };

        let input = inputs.entry(name.clone()).or_insert_with(|| Input {
            name,
            url: String::new(),
        });

        match (parts.next().as_deref(), parts.next()) {
            (Some("url"), None) => {
                if let Some(url) = av.value().and_then(string_value) {
                    input.url = url;
                }
            }
            (None, None) => {
                if let Some(url) = av.value().and_then(extract_nested_url) {
                    input.url = url;
                }
            }
            _ => {}
        }
    }

    inputs.into_values().collect()
}

pub fn add_input(content: &str, name: &str, url: &str) -> String {
    let root = Root::parse(content).tree();

    let attr_set = match root.expr() {
        Some(Expr::AttrSet(s)) => s,
        _ => return content.to_string(),
    };

    let inputs_av = match attr_set
        .entries()
        .filter_map(as_attrpath_value)
        .find(|av| is_key(av, "inputs"))
    {
        Some(av) => av,
        _ => return content.to_string(),
    };

    let inputs_set = match inputs_av.value() {
        Some(Expr::AttrSet(s)) => s,
        _ => return content.to_string(),
    };

    let entries = inputs_set.entries().collect::<Vec<_>>();
    let new_entry = format!("{}.url = \"{}\";", name, url);

    if let Some(last_entry) = entries.last() {
        let node = match last_entry {
            Entry::AttrpathValue(av) => av.syntax(),
            Entry::Inherit(i) => i.syntax(),
        };
        let end_offset: usize = node.text_range().end().into();

        let mut new_content = content.to_string();
        // Find if there's a newline after the last entry
        let after_last = &content[end_offset..];
        let mut insertion_pos = end_offset;
        if let Some(line_end) = after_last.find('\n') {
            insertion_pos += line_end;
        }

        // Find indentation of last entry
        let start_offset: usize = node.text_range().start().into();
        let before_last = &content[..start_offset];
        let last_newline = before_last.rfind('\n').unwrap_or(0);
        let indent_str = if last_newline < before_last.len() {
             &before_last[last_newline + 1..]
        } else {
             ""
        }.chars().take_while(|c: &char| c.is_whitespace()).collect::<String>();

        new_content.insert_str(insertion_pos, &format!("\n{}{}", indent_str, new_entry));
        new_content
    } else {
        // Empty inputs set, insert between braces
        let node = inputs_set.syntax();
        let range = node.text_range();
        let start: usize = range.start().into();
        let end: usize = range.end().into();
        let set_text = &content[start..end];

        if let Some(brace_pos) = set_text.find('{') {
            let mut new_content = content.to_string();
            new_content.insert_str(start + brace_pos + 1, &format!("\n    {}", new_entry));
            new_content
        } else {
            content.to_string()
        }
    }
}

fn as_attrpath_value(entry: Entry) -> Option<AttrpathValue> {
    match entry {
        Entry::AttrpathValue(av) => Some(av),
        _ => None,
    }
}

fn is_key(av: &AttrpathValue, target: &str) -> bool {
    av.attrpath()
        .map(|p| p.to_string().trim() == target)
        .unwrap_or(false)
}

fn string_value(expr: Expr) -> Option<String> {
    match expr {
        Expr::Str(s) => Some(s.to_string().trim_matches('"').to_string()),
        _ => None,
    }
}

fn extract_nested_url(expr: Expr) -> Option<String> {
    let set = match expr {
        Expr::AttrSet(s) => s,
        _ => return None,
    };

    for entry in set.entries() {
        if let Some(av) = match entry {
            Entry::AttrpathValue(av) => Some(av),
            _ => None,
        } && is_key(&av, "url")
            && let Some(url) = av.value().and_then(string_value)
        {
            return Some(url);
        }
    }

    None
}
