use super::process::{ChildRegistry, stdout};
use super::search::SearchPackage;
use super::{Error, Result};
use crate::flake::{Output, OutputKind, Package, files, parse};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub fn flake_show(flake_nix: &Path) -> Result<Vec<Output>> {
    let dir = files::dir(flake_nix);
    let output = Command::new("nix")
        .args([
            "flake",
            "show",
            &files::flake_ref(dir),
            "--json",
            "--impure",
        ])
        .output()
        .map_err(|e| Error::Spawn("nix", e))?;
    if !output.status.success() {
        return Err(Error::Message(format!(
            "Failed to run nix flake show: {}. This might be due to a connection issue or an invalid flake.",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| Error::Parse("nix flake show", e))?;
    let source = std::fs::read_to_string(flake_nix).unwrap_or_default();
    Ok(parse::outputs_from_flake_show(&json, &source))
}

const PACKAGE_INFO_EXPR: &str = r#"p: let
  getPkgInfo = attr: p: let
    tried = builtins.tryEval p;
  in if tried.success && (tried.value ? pname || tried.value ? name) then
    let v = tried.value;
        pname = if v ? pname then v.pname else (builtins.parseDrvName v.name).name;
    in if (pname == "builder.sh") || (pname == "default-builder.sh") || (builtins.match ".*-hook.*" pname != null) then null else {
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
    }
  else null;
  extractList = l: if builtins.isList l then builtins.filter (x: x != null && (x.version or "") != "") (map (x: getPkgInfo "" x) l) else [];
  extract = t: EXTRACT;
in extract p"#;

#[derive(Deserialize)]
struct EvalPackage {
    pname: String,
    attribute: String,
    name: String,
    version: String,
    description: String,
    is_unfree: bool,
}

/// Evaluates the packages an output installs; `source` is left for the caller to resolve.
pub fn output_packages(flake_ref: &str, output: &Output) -> Result<Vec<Package>> {
    let quoted_path = output
        .path
        .split('.')
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(".");
    let attr = output.kind.attr();
    let (target, extract) = match output.kind {
        OutputKind::Nixos | OutputKind::Darwin => (
            format!("{}.{}.config.environment.systemPackages", attr, quoted_path),
            "extractList t",
        ),
        OutputKind::Home => (
            format!("{}.{}", attr, quoted_path),
            "extractList (t.config.home.packages or (t.home.packages or []))",
        ),
        OutputKind::DevShell => (
            format!("{}.{}", attr, quoted_path),
            "extractList ((t.packages or []) ++ (t.buildInputs or []) ++ (t.nativeBuildInputs or []))",
        ),
        OutputKind::Packages | OutputKind::LegacyPackages => return Ok(Vec::new()),
    };

    let installable = format!("{}#{}", flake_ref, target);
    crate::log::action(
        "Evaluating nix expressions",
        format!("nix eval {}", installable),
    );

    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .args(["eval", &installable, "--json", "--impure", "--apply"])
        .arg(PACKAGE_INFO_EXPR.replace("EXTRACT", extract))
        .output()
        .map_err(|e| Error::Spawn("nix", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        crate::log::output(
            "Nix Error",
            format!(
                "{}. This might be due to a connection issue or an evaluation error.",
                stderr
            ),
        );
        return Err(Error::Message(stderr));
    }

    let Ok(evaluated) = serde_json::from_slice::<Vec<EvalPackage>>(&output.stdout) else {
        crate::log::output("Nix Error", "Failed to parse nix eval output");
        return Err(Error::Message(
            "Failed to parse nix eval output".to_string(),
        ));
    };
    crate::log::output(
        "Nix Output",
        format!("Successfully evaluated {} packages", evaluated.len()),
    );

    let mut by_name: HashMap<String, Package> = HashMap::new();
    for pkg in evaluated {
        let name = [pkg.attribute, pkg.pname, pkg.name]
            .into_iter()
            .find(|n| !n.is_empty())
            .unwrap_or_default();
        by_name.insert(
            name.clone(),
            Package {
                name,
                description: pkg.description,
                version: pkg.version,
                is_unfree: pkg.is_unfree,
                source: None,
                attr: None,
            },
        );
    }
    Ok(by_name.into_values().collect())
}

#[derive(Deserialize)]
struct FlakeSearchPackage {
    description: Option<String>,
    version: Option<String>,
}

pub fn search_flake(
    flake_url: &str,
    query: &str,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<SearchPackage>> {
    let mut cmd = Command::new("nix");
    cmd.args(["search", "--json", flake_url, query]);
    let out = stdout("nix search", cmd, reg)?;
    let results: HashMap<String, FlakeSearchPackage> =
        serde_json::from_slice(&out).map_err(|e| Error::Parse("nix search", e))?;

    Ok(results
        .into_iter()
        .map(|(attribute, pkg)| SearchPackage {
            attribute,
            version: pkg.version,
            description: pkg.description,
            platforms: None,
            licenses: None,
            hash: None,
        })
        .collect())
}

/// The version of `attr` at a specific nixpkgs revision.
pub fn version_at(rev: &str, attr: &str, reg: Option<&ChildRegistry>) -> Option<String> {
    let mut cmd = Command::new("nix");
    cmd.args([
        "eval",
        "--json",
        &format!("github:NixOS/nixpkgs/{}#{}.version", rev, attr),
    ]);
    let out = stdout("nix eval", cmd, reg).ok()?;
    serde_json::from_slice(&out).ok()
}

/// Versions of the given attributes in the system `<nixpkgs>`, `"Unknown"` when missing.
pub fn system_versions(attrs: &[String], reg: Option<&ChildRegistry>) -> HashMap<String, String> {
    if attrs.is_empty() {
        return HashMap::new();
    }

    let mut expr = "let pkgs = import <nixpkgs> {}; \
                    lib = pkgs.lib; \
                    getV = pathStr: let \
                      path = lib.splitString \".\" pathStr; \
                      pkg = lib.attrByPath path null pkgs; \
                      res = if pkg != null then builtins.tryEval (pkg.version or \"Unknown\") else { success = false; }; \
                    in if res.success then res.value else \"Unknown\"; \
                    in { "
        .to_string();
    for attr in attrs.iter().filter(|a| {
        a.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    }) {
        expr.push_str(&format!("\"{}\" = getV \"{}\"; ", attr, attr));
    }
    expr.push('}');

    let mut cmd = Command::new("nix");
    cmd.args(["eval", "--json", "--impure", "--expr", &expr]);
    stdout("nix eval", cmd, reg)
        .ok()
        .and_then(|out| serde_json::from_slice(&out).ok())
        .unwrap_or_default()
}

pub struct Metadata {
    pub version: String,
    pub description: String,
}

/// Looks `attr` up in the user's channel with `nix-env -qa`.
pub fn metadata(attr: &str) -> Option<Metadata> {
    let output = Command::new("nix-env")
        .args(["-qa", attr, "--json"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let meta = json.as_object()?.values().next()?;
    Some(Metadata {
        version: meta
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string(),
        description: meta
            .pointer("/meta/description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string(),
    })
}
