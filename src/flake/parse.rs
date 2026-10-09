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
    let root = Root::parse(flake_nix).syntax();
    let Some(mut body) = root
        .children()
        .find(|c| c.kind() == SyntaxKind::NODE_ATTR_SET)
        .and_then(|set| {
            set.children()
                .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
                .find(|c| binding_path(c).as_deref() == Some("outputs"))
        })
        .and_then(|b| binding_value(&b))
    else {
        return Vec::new();
    };

    let mut lets: HashMap<String, String> = HashMap::new();
    while matches!(
        body.kind(),
        SyntaxKind::NODE_LAMBDA
            | SyntaxKind::NODE_LET_IN
            | SyntaxKind::NODE_WITH
            | SyntaxKind::NODE_PAREN
    ) {
        if body.kind() == SyntaxKind::NODE_LET_IN {
            for binding in body
                .children()
                .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
            {
                if let (Some(key), Some(val)) = (binding_path(&binding), binding_value(&binding)) {
                    lets.insert(key, val.to_string().trim().trim_matches('"').to_string());
                }
            }
        }
        let Some(inner) = body.children().last() else {
            return Vec::new();
        };
        body = inner;
    }

    let mut result = Vec::new();
    if body.kind() == SyntaxKind::NODE_ATTR_SET {
        collect_outputs(&body, &[], &lets, &mut result);
    }
    result
}

/// Walks nested sets until the path reaches an output, e.g. `devShells.<system>.<name>`.
fn collect_outputs(
    set: &SyntaxNode,
    prefix: &[String],
    lets: &HashMap<String, String>,
    result: &mut Vec<Output>,
) {
    for binding in set
        .children()
        .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
    {
        let (Some(attrpath), Some(value)) = (
            binding
                .children()
                .find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH),
            binding_value(&binding),
        ) else {
            continue;
        };
        let path: Vec<String> = prefix
            .iter()
            .cloned()
            .chain(attr_segments(&attrpath))
            .collect();
        let Some(kind) = path.first().and_then(|p| OutputKind::from_attr(p)) else {
            continue;
        };
        let depth = if kind.is_per_system() { 3 } else { 2 };
        if path.len() < depth && value.kind() == SyntaxKind::NODE_ATTR_SET {
            collect_outputs(&value, &path, lets, result);
        } else if path.len() == depth {
            result.push(Output {
                kind,
                path: path[1..]
                    .iter()
                    .map(|s| resolve_segment(s, lets))
                    .collect::<Vec<_>>()
                    .join("."),
                name: output_name(&value),
                content: Some(value.to_string()),
            });
        }
    }
}

fn resolve_segment(segment: &str, lets: &HashMap<String, String>) -> String {
    let segment = segment.trim_matches('"');
    segment
        .strip_prefix("${")
        .and_then(|s| s.strip_suffix('}'))
        .and_then(|name| lets.get(name.trim()))
        .cloned()
        .unwrap_or_else(|| segment.to_string())
}

/// The `name` set on an output, e.g. in `pkgs.mkShell { name = "..."; }`.
fn output_name(value: &SyntaxNode) -> Option<String> {
    value
        .descendants()
        .find(|n| n.kind() == SyntaxKind::NODE_ATTR_SET)?
        .children()
        .filter(|c| c.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
        .find(|b| binding_path(b).as_deref() == Some("name"))
        .and_then(|b| binding_value(&b))
        .map(|v| v.to_string().trim_matches('"').to_string())
}

pub fn attr_segments(attrpath: &SyntaxNode) -> Vec<String> {
    attrpath
        .children_with_tokens()
        .filter(|c| {
            !matches!(
                c.kind(),
                SyntaxKind::TOKEN_DOT | SyntaxKind::TOKEN_WHITESPACE
            )
        })
        .map(|c| c.to_string().trim().to_string())
        .collect()
}

/// The input a package reference comes from and the attribute it names, e.g.
/// `inputs.foo.legacyPackages.${system}.hello` gives `(Some("foo"), "hello")`.
pub fn ref_source(attr: &str) -> (Option<&str>, &str) {
    let rest = attr.strip_prefix("inputs.").unwrap_or(attr);
    let Some((source, rest)) = rest.split_once('.') else {
        return (None, attr);
    };
    let leaf = ["legacyPackages.", "packages."]
        .iter()
        .find_map(|p| rest.strip_prefix(p))
        .and_then(|r| r.split_once('.').map(|(_, leaf)| leaf))
        .unwrap_or(rest);
    (Some(source), leaf)
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
