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
        
        let config_attr = if config_type == "devShells" {
            let parts: Vec<&str> = config_name.split('.').collect();
            if parts.len() == 2 {
                format!("{}.\"{}\".\"{}\"", config_type, parts[0], parts[1])
            } else {
                format!("{}.\"{}\"", config_type, config_name)
            }
        } else {
            format!("{}.\"{}\"", config_type, config_name)
        };

        let target_attr = match config_type {
            "nixosConfigurations" => format!("{}.config.environment.systemPackages", config_attr),
            "homeConfigurations" => format!("{}.config.home.packages", config_attr),
            "darwinConfigurations" => format!("{}.config.environment.systemPackages", config_attr),
            "devShells" => config_attr,
            _ => return Ok(results),
        };

        let apply_expr = "p: let
          getPkgInfo = p: let 
            tried = builtins.tryEval p;
          in if tried.success && (p ? pname || p ? name) then {
            pname = p.pname or (builtins.parseDrvName p.name).name;
            name = p.name or \"\";
            version = p.version or (builtins.parseDrvName p.name).version;
            description = p.meta.description or \"\";
          } else null;
          extract = t: if builtins.isList t then
            builtins.filter (x: x != null) (map getPkgInfo t)
          else if builtins.isAttrs t then
            if t ? buildInputs || t ? nativeBuildInputs || t ? packages then
              (extract (t.packages or [])) ++ (extract (t.buildInputs or [])) ++ (extract (t.nativeBuildInputs or []))
            else
              []
          else [];
        in extract p";

        let mut command = Command::new("nix");
        command.args([
            "eval",
            &format!("{}#{}", flake_path, target_attr),
            "--json",
            "--impure",
            "--apply",
            apply_expr,
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
                    results.insert(pkg.pname.clone(), (pkg.description.clone(), pkg.version.clone()));
                    results.insert(pkg.name, (pkg.description, pkg.version));
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
