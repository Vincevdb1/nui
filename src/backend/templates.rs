use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub description: String,
}

pub fn dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("nui").join("templates"))
}

pub fn ensure_dir() {
    if let Some(dir) = dir() {
        let _ = std::fs::create_dir_all(dir);
    }
}

pub fn path(name: &str) -> Option<PathBuf> {
    Some(dir()?.join(name))
}

pub fn list() -> Vec<Template> {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let description = std::fs::read_to_string(e.path())
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find(|l| l.trim().starts_with("description"))
                        .and_then(|l| l.split('"').nth(1))
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "No description".to_string());
            Some(Template { name, description })
        })
        .collect()
}

/// Package attributes listed in a template's package lists, without the `pkgs.` prefix.
pub fn packages(content: &str) -> Vec<String> {
    let mut packages: Vec<String> = Vec::new();
    for pattern in ["packages = [", "buildInputs = [", "nativeBuildInputs = ["] {
        let mut rest = content;
        while let Some(start) = rest.find(pattern) {
            let after = &rest[start + pattern.len()..];
            let Some(end) = after.find("];") else {
                break;
            };
            for item in after[..end].split_whitespace() {
                let pkg = item
                    .strip_prefix("pkgs.")
                    .unwrap_or(item)
                    .trim_matches(|c| matches!(c, '"' | '\'' | ';' | '[' | ']' | '(' | ')'));
                let keep = !pkg.is_empty()
                    && pkg != "with"
                    && pkg != "pkgs"
                    && !pkg.starts_with("self.")
                    && !pkg.contains("${");
                if keep && !packages.iter().any(|p| p == pkg) {
                    packages.push(pkg.to_string());
                }
            }
            rest = &after[end + 2..];
        }
    }
    packages
}

/// Writes a flake with a default devShell holding `packages`; returns the file name.
pub fn save_shell(name: &str, description: &str, packages: &[String]) -> std::io::Result<String> {
    let mut filename = name.to_string();
    if !filename.ends_with(".nix") {
        filename.push_str(".nix");
    }
    let path = path(&filename).ok_or_else(|| std::io::Error::other("no config directory"))?;

    let package_list: String = packages.iter().map(|p| format!("pkgs.{} ", p)).collect();
    let content = format!(
        "{{ \n  description = \"{description}\";\n  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n  outputs = {{ self, nixpkgs }}: let system = \"x86_64-linux\"; pkgs = nixpkgs.legacyPackages.${{system}}; in {{ \n    devShells.${{system}}.default = pkgs.mkShell {{ \n      packages = [ {package_list}]; \n    }}; \n  }};\n}}"
    );
    std::fs::write(path, content)?;
    Ok(filename)
}
