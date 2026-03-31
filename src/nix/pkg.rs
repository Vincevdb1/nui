use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
}

impl Package {
    pub fn fetch_from_config(flake_path: &str, config_type: &str, config_name: &str) -> Result<HashMap<String, (String, String)>, String> {
        let mut results = HashMap::new();
        
        let target_attr = if config_type == "devShells" {
            let parts: Vec<&str> = config_name.split('.').collect();
            if parts.len() == 2 {
                format!("{}.\"{}\".\"{}\"", config_type, parts[0], parts[1])
            } else {
                format!("{}.\"{}\"", config_type, config_name)
            }
        } else if matches!(config_type, "nixosConfigurations" | "homeConfigurations" | "darwinConfigurations") {
            format!("{}.\"{}\"", config_type, config_name)
        } else {
            return Ok(results);
        };

        let extract_logic = match config_type {
            "nixosConfigurations" | "darwinConfigurations" => 
                "extractList (t.config.environment.systemPackages or [])",
            "homeConfigurations" => 
                "extractList (t.config.home.packages or (t.home.packages or []))",
            "devShells" => 
                "extractList (t.buildInputs or [])",
            _ => "extractList (if builtins.isList l then l else [])"
        };

        let apply_expr = format!(r#"p: let
          getPkgInfo = p: let 
            tried = builtins.tryEval p;
          in if tried.success && (p ? pname || p ? name) then {{
            pname = if p ? pname then p.pname else (builtins.parseDrvName p.name).name;
            name = p.name or "";
            version = p.version or (builtins.parseDrvName p.name).version;
            description = p.meta.description or "";
          }} else null;
          extractList = l: if builtins.isList l then builtins.filter (x: x != null) (map getPkgInfo l) else [];
          extract = t: {};
        in extract p"#, extract_logic);

        let mut command = Command::new("nix");
        let attr_path = format!("{}#{}", flake_path, target_attr);
        crate::command_log(format!("Evaluating nix expressions for {}...", attr_path));
        
        command.args([
            "eval",
            &attr_path,
            "--json",
            "--impure",
            "--apply",
            &apply_expr,
        ]);

        let output = command.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            #[derive(Deserialize)]
            struct EvalPkg {
                pname: String,
                name: String,
                version: String,
                description: String,
            }

            if let Ok(eval_results) = serde_json::from_slice::<Vec<EvalPkg>>(&output.stdout) {
                for pkg in eval_results {
                    let key = if !pkg.pname.is_empty() {
                        pkg.pname
                    } else {
                        pkg.name
                    };
                    results.insert(key, (pkg.description, pkg.version));
                }
                Ok(results)
            } else {
                Err("Failed to parse nix eval output".to_string())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr.to_string())
        }
    }
}
