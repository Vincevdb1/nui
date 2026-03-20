use rnix::{
    ast::{self, AttrpathValue, Expr, HasEntry},
    Root,
};

pub fn extract_inputs(content: &str) -> Vec<String> {
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

    // Collect keys
    let inputs: Vec<String> = inner_set
        .entries()
        .filter_map(|entry| {
            if let ast::Entry::AttrpathValue(av) = entry {
                av.attrpath()?
                    .attrs()
                    .next()
                    .map(|a| a.to_string().trim().to_string())
            } else {
                None
            }
        })
        .collect();

    inputs
}

fn is_key(av: &AttrpathValue, target: &str) -> bool {
    av.attrpath()
        .map(|p| p.to_string().trim() == target)
        .unwrap_or(false)
}
