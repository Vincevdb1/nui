use rnix::{
    ast::{self, AttrpathValue, Expr, HasEntry},
    Root,
};
use crate::nix::Input;

pub fn extract_inputs(content: &str) -> Vec<Input> {
    let root = Root::parse(content).tree();

    // Get top-level attr set
    let attr_set = match root.expr() {
        Some(Expr::AttrSet(s)) => s,
        _ => return vec![],
    };

    // Find "inputs" attribute
    let inputs_attr = attr_set
        .entries()
        .filter_map(|e| match e {
            ast::Entry::AttrpathValue(av) => Some(av),
            _ => None,
        })
        .find(|av| is_key(av, "inputs"));

    // Get inner attr set
    let inner_set = match inputs_attr.and_then(|av| av.value()) {
        Some(Expr::AttrSet(s)) => s,
        _ => return vec![],
    };

    // Collect inputs
    let mut inputs: Vec<Input> = Vec::new();

    for entry in inner_set.entries() {
        if let ast::Entry::AttrpathValue(av) = entry {
            let path: Vec<String> = av.attrpath()
                .map(|p| p.attrs().map(|a| a.to_string().trim().to_string()).collect())
                .unwrap_or_default();
            
            if path.is_empty() { continue; }
            
            let name = path[0].clone();
            let index = if let Some(idx) = inputs.iter().position(|i| i.name == name) {
                idx
            } else {
                inputs.push(Input {
                    name: name.clone(),
                    url: String::new(),
                });
                inputs.len() - 1
            };

            let input = &mut inputs[index];

            if path.len() == 2 && path[1] == "url" {
                if let Some(value) = av.value() {
                    if let Expr::Str(s) = value {
                        input.url = s.to_string().trim_matches('"').to_string();
                    }
                }
            } else if path.len() == 1 {
                if let Some(value) = av.value() {
                    match value {
                        Expr::AttrSet(s) => {
                            // Look for url inside the set
                            for inner_entry in s.entries() {
                                if let ast::Entry::AttrpathValue(inner_av) = inner_entry {
                                    if is_key(&inner_av, "url") {
                                        if let Some(Expr::Str(url_str)) = inner_av.value() {
                                            input.url = url_str.to_string().trim_matches('"').to_string();
                                        }
                                    }
                                }
                            }
                        }
                        Expr::Str(s) => {
                            // Direct assignment: name = "url"
                            input.url = s.to_string().trim_matches('"').to_string();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    inputs
}

fn is_key(av: &AttrpathValue, target: &str) -> bool {
    av.attrpath()
        .map(|p| p.to_string().trim() == target)
        .unwrap_or(false)
}
