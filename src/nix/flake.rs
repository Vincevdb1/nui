use crate::nix::{Input, Configuration};
use std::collections::HashMap;
use rnix::Root;

pub fn extract_inputs(content: &str) -> Vec<Input> {
    let mut inputs: HashMap<String, Input> = HashMap::new();

    if let Ok(collection) = nix_editor::parse::get_collection(content.to_string()) {
        for (key, val) in collection {
            if let Some(rest) = key.strip_prefix("inputs.") {
                if let Some(name) = rest.strip_suffix(".url") {
                    let url = val.trim_matches('"').to_string();
                    inputs.insert(name.to_string(), Input {
                        name: name.to_string(),
                        url,
                    });
                }
            }
        }
    }

    inputs.into_values().collect()
}

pub fn extract_configurations(content: &str) -> Vec<Configuration> {
    let mut configs = Vec::new();

    if let Ok(outputs_val) = nix_editor::read::readvalue(content, "outputs") {
        let mut replacements: HashMap<String, String> = HashMap::new();
        
        // Extract let bindings for replacements
        let ast = Root::parse(&outputs_val);
        use rnix::SyntaxKind;
        for node in ast.syntax().descendants() {
            if node.kind() == SyntaxKind::NODE_LET_IN {
                for child in node.children() {
                    if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
                        let mut key = String::new();
                        let mut val = String::new();
                        if let Some(path) = child.children().find(|c| c.kind() == SyntaxKind::NODE_ATTRPATH) {
                            key = path.text().to_string().trim().to_string();
                        }
                        if let Some(v) = child.children().find(|c| c.kind() != SyntaxKind::NODE_ATTRPATH) {
                            val = v.text().to_string().trim().trim_matches('"').to_string();
                        }
                        if !key.is_empty() && !val.is_empty() {
                            replacements.insert(format!("${{{}}}", key), val);
                        }
                    }
                }
            }
        }

        if let Ok(collection) = nix_editor::parse::get_collection(outputs_val) {
            for (key, val) in collection {
                let parts: Vec<&str> = key.split('.').collect();
                if !parts.is_empty() {
                    let config_type = parts[0];
                    if matches!(config_type, "nixosConfigurations" | "homeConfigurations" | "devShells" | "darwinConfigurations") {
                        let mut path = parts[1..].join(".");
                        
                        // Apply replacements
                        for (k, v) in &replacements {
                            path = path.replace(k, v);
                        }

                        let mut name_opt = None;
                        
                        if let Ok(name_val) = nix_editor::read::readvalue(&val, "name") {
                            name_opt = Some(name_val.trim_matches('"').to_string());
                        }

                        configs.push(Configuration {
                            path,
                            name: name_opt,
                            config_type: config_type.to_string(),
                        });
                    }
                }
            }
        }
    }

    configs
}

pub fn add_input(content: &str, name: &str, url: &str) -> String {
    let query = format!("inputs.{}.url", name);
    let value = format!("\"{}\"", url);
    match nix_editor::write::write(content, &query, &value) {
        Ok(new_content) => new_content,
        Err(_) => content.to_string(),
    }
}
