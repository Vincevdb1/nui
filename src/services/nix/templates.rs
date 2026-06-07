use super::NixService;
use crate::action::Action;
use crate::state::Mode;

impl NixService {
    pub fn apply_template(&self, template_path: std::path::PathBuf, mode: Mode) {
        let tx = self.tx.clone();
        if mode == Mode::Shell {
            match std::fs::read_to_string(&template_path) {
                Ok(content) => {
                    let pkgs = self.extract_packages_from_template(&content);
                    let _ = tx.send(Action::ApplyShellTemplate(pkgs));
                }
                Err(e) => {
                    crate::log_output("Error", format!("Failed to read template: {}", e));
                }
            }
        } else {
            match std::fs::copy(&template_path, "flake.nix") {
                Ok(_) => {
                    crate::log_output("Success", format!("Applied template to flake.nix"));
                    let _ = tx.send(Action::RefreshContext);
                }
                Err(e) => {
                    crate::log_output("Error", format!("Failed to apply template: {}", e));
                }
            }
        }
    }

    pub fn load_templates(&self) -> Vec<(String, String)> {
        let mut templates = Vec::new();
        if let Some(config_dir) = dirs::config_dir() {
            let template_dir = config_dir.join("nui").join("templates");
            if let Ok(entries) = std::fs::read_dir(template_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
                            let path = entry.path();
                            if let Some(name) = entry.file_name().to_str() {
                                let description =
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        content
                                            .lines()
                                            .find(|l| l.trim().starts_with("description"))
                                            .and_then(|l| l.split('"').nth(1))
                                            .unwrap_or("No description")
                                            .to_string()
                                    } else {
                                        "No description".to_string()
                                    };
                                templates.push((name.to_string(), description));
                            }
                        }
                    }
                }
            }
        }
        templates
    }

    pub fn extract_packages_from_template(&self, content: &str) -> Vec<String> {
        let mut packages = Vec::new();

        let patterns = ["packages = [", "buildInputs = [", "nativeBuildInputs = ["];

        for pattern in patterns {
            let mut current_pos = 0;
            while let Some(start) = content[current_pos..].find(pattern) {
                let actual_start = current_pos + start + pattern.len();
                let rest = &content[actual_start..];
                if let Some(end) = rest.find("];") {
                    let list = &rest[..end];
                    for item in list.split_whitespace() {
                        let mut pkg = item;
                        if let Some(stripped) = pkg.strip_prefix("pkgs.") {
                            pkg = stripped;
                        }
                        let clean_pkg = pkg.trim_matches(|c| {
                            c == '"'
                                || c == '\''
                                || c == ';'
                                || c == '['
                                || c == ']'
                                || c == '('
                                || c == ')'
                        });

                        if !clean_pkg.is_empty()
                            && clean_pkg != "with"
                            && clean_pkg != "pkgs"
                            && !clean_pkg.starts_with("self.")
                            && !clean_pkg.contains("${")
                        {
                            if !packages.contains(&clean_pkg.to_string()) {
                                packages.push(clean_pkg.to_string());
                            }
                        }
                    }
                    current_pos = actual_start + end + 2;
                } else {
                    break;
                }
            }
        }
        packages
    }
}
