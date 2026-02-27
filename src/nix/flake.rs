use rnix::{ast::{self, AttrpathValue, Expr, HasEntry}, Root};
use std::collections::HashSet;

pub fn extract_inputs(content: &str) -> Vec<String> {
    let root = Root::parse(content).tree();
    
    let inputs = root.expr()
        .and_then(|expr| match expr { Expr::AttrSet(s) => Some(s), _ => None })
        .into_iter()
        .flat_map(|attr_set| attr_set.entries())
        .filter_map(|entry| match entry { ast::Entry::AttrpathValue(av) => Some(av), _ => None })
        .find(|av| is_key(av, "inputs"))
        .and_then(|av| av.value())
        .and_then(|val| match val { Expr::AttrSet(s) => Some(s), _ => None })
        .into_iter()
        .flat_map(|inner_set| inner_set.entries())
        .filter_map(|entry| {
            if let ast::Entry::AttrpathValue(av) = entry {
                av.attrpath()?.attrs().next().map(|a| a.to_string().trim().to_string())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let result: Vec<_> = inputs.into_iter().collect();
    result
}

fn is_key(av: &AttrpathValue, target: &str) -> bool {
    av.attrpath()
        .map(|p| p.to_string().trim() == target)
        .unwrap_or(false)
}
