use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub is_unfree: bool,
    pub source_input: Option<String>,
}

impl Package {
    pub fn fetch_from_output(
        flake_path: &str,
        output_type: &str,
        output_name: &str,
    ) -> Result<HashMap<String, (String, String, bool, String)>, String> {
        let mut results = HashMap::new();

        let quoted_name = output_name
            .split('.')
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(".");

        let (target_attr, extract_logic) = match output_type {
            "nixosConfigurations" | "darwinConfigurations" => (
                format!("{}.{}.config.environment.systemPackages", output_type, quoted_name),
                "extractList t",
            ),
            "homeConfigurations" => (
                format!("{}.{}", output_type, quoted_name),
                "extractList (t.config.home.packages or (t.home.packages or []))",
            ),
            "devShells" => (
                format!("{}.{}", output_type, quoted_name),
                "extractList ((t.packages or []) ++ (t.buildInputs or []) ++ (t.nativeBuildInputs or []))",
            ),
            _ => return Ok(results),
        };

        let apply_expr = format!(
            r#"p: let
          getPkgInfo = attr: p: let 
            tried = builtins.tryEval p;
          in if tried.success && (tried.value ? pname || tried.value ? name) then
            let v = tried.value; 
                pname = if v ? pname then v.pname else (builtins.parseDrvName v.name).name;
            in if (pname == "builder.sh") || (pname == "default-builder.sh") || (builtins.match ".*-hook.*" pname != null) then null else {{
              pname = pname;
              attribute = attr;
              name = v.name or "";
              version = v.version or (builtins.parseDrvName v.name).version;
              description = v.meta.description or "";
              is_unfree = if v ? meta && v.meta ? license then 
                let 
                  lic = v.meta.license;
                  isUnfree = l: if builtins.isAttrs l then l.free or true == false else false;
                in if builtins.isList lic then builtins.any isUnfree lic else isUnfree lic
              else false;
            }}
          else null;
          extractList = l: if builtins.isList l then builtins.filter (x: x != null && (x.version or "") != "") (map (x: getPkgInfo "" x) l) else [];
          extractMap = m: if builtins.isAttrs m then builtins.filter (x: x != null) (builtins.attrValues (builtins.mapAttrs getPkgInfo m)) else [];
          extract = t: {};
        in extract p"#,
            extract_logic
        );


        let attr_path = format!("{}#{}", crate::nix::flake::normalize_flake_ref(flake_path), target_attr);

        crate::log_action(
            "Evaluating nix expressions",
            format!("nix eval {}", attr_path),
        );

        let mut command = Command::new("nix");
        command.env("NIXPKGS_ALLOW_UNFREE", "1");
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
                attribute: String,
                name: String,
                version: String,
                description: String,
                is_unfree: bool,
            }

            if let Ok(eval_results) = serde_json::from_slice::<Vec<EvalPkg>>(&output.stdout) {
                let count = eval_results.len();
                for pkg in eval_results {
                    let key = if !pkg.attribute.is_empty() {
                        pkg.attribute
                    } else if !pkg.pname.is_empty() {
                        pkg.pname
                    } else {
                        pkg.name
                    };
                    results.insert(key, (pkg.description, pkg.version, pkg.is_unfree, String::new()));
                }
                crate::log_output(
                    "Nix Output",
                    format!("Successfully evaluated {} packages", count),
                );
                Ok(results)
            } else {
                crate::log_output("Nix Error", "Failed to parse nix eval output");
                Err("Failed to parse nix eval output".to_string())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            crate::log_output("Nix Error", format!("{}. This might be due to a connection issue or an evaluation error.", stderr));
            Err(stderr.to_string())
        }
    }
}
