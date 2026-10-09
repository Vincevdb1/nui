use super::model::{Input, Output, OutputKind};
use rnix::{Root, SyntaxKind, SyntaxNode};
use serde::Deserialize;
use std::collections::HashMap;

pub const PACKAGE_LIST_ATTRS: [&str; 3] = ["packages", "buildInputs", "nativeBuildInputs"];

#[derive(Deserialize)]
struct LockFile {
    nodes: HashMap<String, LockNode>,
}

#[derive(Deserialize)]
struct LockNode {
    locked: Option<LockLocked>,
    original: Option<LockOriginal>,
}

#[derive(Deserialize)]
struct LockLocked {
    rev: Option<String>,
}

#[derive(Deserialize)]
struct LockOriginal {
    #[serde(rename = "ref")]
    branch: Option<String>,
}

pub fn inputs(flake_nix: &str, flake_lock: Option<&str>) -> Vec<Input> {
    let lock: Option<LockFile> = flake_lock.and_then(|c| serde_json::from_str(c).ok());
    let Ok(collection) = nix_editor::parse::get_collection(flake_nix.to_string()) else {
        return Vec::new();
    };

    let mut inputs: HashMap<String, Input> = HashMap::new();
    for (key, val) in collection {
        let Some(name) = key
            .strip_prefix("inputs.")
            .and_then(|rest| rest.strip_suffix(".url"))
        else {
            continue;
        };
        let node = lock.as_ref().and_then(|lock| lock.nodes.get(name));
        inputs.insert(
            name.to_string(),
            Input {
                name: name.to_string(),
                url: val.trim_matches('"').to_string(),
                branch: node.and_then(|n| n.original.as_ref()?.branch.clone()),
                rev: node.and_then(|n| n.locked.as_ref()?.rev.clone()),
            },
        );
    }
    let mut inputs: Vec<Input> = inputs.into_values().collect();
    inputs.sort_by(|a, b| a.name.cmp(&b.name));
    inputs
}

/// Merges the outputs nix reports with the source text found for them in flake.nix.
pub fn outputs_from_flake_show(json: &serde_json::Value, flake_nix: &str) -> Vec<Output> {
    let ast_outputs = outputs(flake_nix);
    let with_source = |kind: OutputKind, path: String| {
        let found = ast_outputs
            .iter()
            .find(|o| o.kind == kind && o.path == path);
        Output {
            kind,
            name: found.and_then(|o| o.name.clone()),
            content: found.and_then(|o| o.content.clone()),
            path,
        }
    };

    let is_v2 = json.get("version").and_then(|v| v.as_u64()) == Some(2);
    let children = |val: &serde_json::Value| -> Option<serde_json::Map<String, serde_json::Value>> {
        if is_v2 {
            val.get("children")?.as_object().cloned()
        } else {
            val.as_object().cloned()
        }
    };
    let root = if is_v2 {
        json.get("inventory").and_then(|i| i.as_object())
    } else {
        json.as_object()
    };

    let mut result = Vec::new();
    for (attr, val) in root.into_iter().flatten() {
        let Some(kind) = OutputKind::from_attr(attr).filter(|k| {
            matches!(
                k,
                OutputKind::Nixos | OutputKind::Home | OutputKind::DevShell | OutputKind::Darwin
            )
        }) else {
            continue;
        };
        let top = if is_v2 {
            val.get("output").and_then(children)
        } else {
            val.as_object().cloned()
        };
        for (path, inner) in top.into_iter().flatten() {
            if kind.is_per_system() {
                for (name, _) in children(&inner).into_iter().flatten() {
                    result.push(with_source(kind, format!("{}.{}", path, name)));
                }
            } else {
                result.push(with_source(kind, path));
            }
        }
    }

    result.sort_by(|a, b| {
        let default_shell =
            |o: &Output| o.kind == OutputKind::DevShell && o.path.ends_with(".default");
        default_shell(b)
            .cmp(&default_shell(a))
            .then((b.kind == OutputKind::DevShell).cmp(&(a.kind == OutputKind::DevShell)))
            .then(a.kind.attr().cmp(b.kind.attr()))
            .then(a.path.cmp(&b.path))
    });
    result
}

/// Outputs as written in flake.nix, with `let` bindings substituted into their paths.
pub fn outputs(flake_nix: &str) -> Vec<Output> {
    let Ok(outputs_val) = nix_editor::read::readvalue(flake_nix, "outputs") else {
        return Vec::new();
    };

    let mut replacements: HashMap<String, String> = HashMap::new();
    let ast = Root::parse(&outputs_val);
    let mut body_val = outputs_val.clone();

    if let Some(lambda) = ast
        .syntax()
        .descendants()
        .find(|n| n.kind() == SyntaxKind::NODE_LAMBDA)
        && let Some(body) = lambda.children().find(|c| {
            !matches!(
                c.kind(),
                SyntaxKind::NODE_PATTERN
                    | SyntaxKind::TOKEN_COLON
                    | SyntaxKind::TOKEN_WHITESPACE
                    | SyntaxKind::TOKEN_COMMENT
            )
        })
    {
        body_val = body.to_string();
        if body.kind() == SyntaxKind::NODE_LET_IN {
            for binding in body
                .children()
                .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
            {
                let key = binding
                    .children()
                    .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
                    .map(|p| p.text().to_string().trim().to_string())
                    .unwrap_or_default();
                let val = binding
                    .children()
                    .find(|c| c.kind() != SyntaxKind::NODE_ATTRPATH)
                    .map(|v| v.text().to_string().trim().trim_matches('"').to_string())
                    .unwrap_or_default();
                if !key.is_empty() && !val.is_empty() {
                    replacements.insert(format!("${{{}}}", key), val);
                }
            }
            if let Some(actual_body) = body.children().last() {
                body_val = actual_body.to_string();
            }
        }
    }

    let Ok(collection) = nix_editor::parse::get_collection(body_val) else {
        return Vec::new();
    };

    let read_name = |val: &str| {
        nix_editor::read::readvalue(val, "name")
            .ok()
            .map(|n| n.trim_matches('"').to_string())
    };

    let mut result = Vec::new();
    for (key, val) in collection {
        let parts = split_attr_path(&key);
        let Some(kind) = OutputKind::from_attr(&parts[0]) else {
            continue;
        };

        let mut path = parts[1..]
            .iter()
            .take_while(|p| !PACKAGE_LIST_ATTRS.contains(&p.as_str()))
            .map(|p| p.trim_matches('"'))
            .collect::<Vec<_>>()
            .join(".");
        for (k, v) in &replacements {
            path = path.replace(k, v);
        }

        match nix_editor::parse::get_collection(val.clone()) {
            Ok(inner) => {
                for (inner_key, inner_val) in inner {
                    let inner_key = inner_key.trim_matches('"');
                    result.push(Output {
                        kind,
                        path: if path.is_empty() {
                            inner_key.to_string()
                        } else {
                            format!("{}.{}", path, inner_key)
                        },
                        name: read_name(&inner_val),
                        content: Some(inner_val),
                    });
                }
            }
            Err(_) => result.push(Output {
                kind,
                path,
                name: read_name(&val),
                content: Some(val),
            }),
        }
    }
    result
}

fn split_attr_path(key: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in key.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '.' if !in_quotes => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
}

/// The value node of an `attrpath = value;` binding.
pub fn binding_value(binding: &SyntaxNode) -> Option<SyntaxNode> {
    binding.children().find(|c| {
        !matches!(
            c.kind(),
            SyntaxKind::NODE_ATTRPATH | SyntaxKind::TOKEN_COMMENT | SyntaxKind::TOKEN_WHITESPACE
        )
    })
}

pub fn binding_path(binding: &SyntaxNode) -> Option<String> {
    binding
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH)
        .map(|p| p.to_string().trim().to_string())
}

/// The list in `packages = [ ... ]` or `packages = with pkgs; [ ... ]`.
pub fn package_list(value: SyntaxNode) -> Option<SyntaxNode> {
    match value.kind() {
        SyntaxKind::NODE_LIST => Some(value),
        SyntaxKind::NODE_WITH => value.children().find(|c| c.kind() == SyntaxKind::NODE_LIST),
        _ => None,
    }
}

/// Package attributes listed in an output's source, e.g. `pkgs.git`.
pub fn package_attributes(content: &str) -> Vec<String> {
    let ast = Root::parse(content);
    let item_texts = |list: SyntaxNode| {
        list.children()
            .map(|item| item.to_string().trim().to_string())
            .collect::<Vec<_>>()
    };

    let attrs: Vec<String> = ast
        .syntax()
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .filter(|n| binding_path(n).is_some_and(|p| PACKAGE_LIST_ATTRS.contains(&p.as_str())))
        .filter_map(|n| binding_value(&n).and_then(package_list))
        .flat_map(item_texts)
        .collect();

    if !attrs.is_empty() {
        return attrs;
    }
    ast.syntax()
        .children()
        .filter(|n| n.kind() == SyntaxKind::NODE_LIST)
        .flat_map(item_texts)
        .collect()
}
