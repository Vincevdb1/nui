use crate::nix::Input;
use std::collections::HashMap;

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

pub fn add_input(content: &str, name: &str, url: &str) -> String {
    let query = format!("inputs.{}.url", name);
    let value = format!("\"{}\"", url);
    match nix_editor::write::write(content, &query, &value) {
        Ok(new_content) => new_content,
        Err(_) => content.to_string(),
    }
}
