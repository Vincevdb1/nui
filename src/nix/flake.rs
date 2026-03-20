use crate::nix::Input;
use rnix::{
    Root,
    ast::{self, AttrpathValue, Expr, HasEntry},
};
use std::collections::HashMap;

pub fn extract_inputs(content: &str) -> Vec<Input> {
    let root = Root::parse(content).tree();

    let attr_set = match root.expr() {
        Some(Expr::AttrSet(s)) => s,
        _ => return vec![],
    };

    let inputs_set = match attr_set
        .entries()
        .filter_map(as_attrpath_value)
        .find(|av| is_key(av, "inputs"))
        .and_then(|av| av.value())
    {
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

fn as_attrpath_value(entry: ast::Entry) -> Option<AttrpathValue> {
    match entry {
        ast::Entry::AttrpathValue(av) => Some(av),
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
            ast::Entry::AttrpathValue(av) => Some(av),
            _ => None,
        } && is_key(&av, "url")
            && let Some(url) = av.value().and_then(string_value)
        {
            return Some(url);
        }
    }

    None
}
