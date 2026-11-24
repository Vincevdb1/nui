use super::process::{self, ChildRegistry, stdout};
use super::search::SearchPackage;
use super::{Error, Result};
use crate::flake::edit;
use crate::flake::locate::{Fs, Sources};
use crate::flake::{Output, OutputKind, Package, ShellTarget, files, parse};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub fn flake_show(flake_nix: &Path) -> Result<Vec<Output>> {
    let dir = files::dir(flake_nix);
    let output = process::output(Command::new("nix").args([
        "flake",
        "show",
        &files::flake_ref(dir),
        "--json",
        "--impure",
    ]))
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
    let mut outputs = parse::outputs_from_flake_show(&json, &source);
    attach_shell_sources(&Fs, flake_nix, &mut outputs);
    Ok(outputs)
}

/// devShell content comes from the same locator the edits use, so the view and edits agree.
pub fn attach_shell_sources(sources: &dyn Sources, flake_nix: &Path, outputs: &mut [Output]) {
    for output in outputs {
        let Some(target) = output.shell_target() else {
            continue;
        };
        // The items as one list literal, which `parse::package_attributes` reads.
        match edit::shell_packages(sources, flake_nix, &target) {
            Ok(items) => output.content = Some(format!("[ {} ]", items.join(" "))),
            Err(e) => output.edit_error = Some(e.to_string()),
        }
    }
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

/// The flake attribute to evaluate for an output and the expression that extracts its
/// package list; `None` for outputs that don't install packages.
pub fn eval_target(output: &Output) -> Option<(String, &'static str)> {
    let quoted_path = output
        .path
        .split('.')
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(".");
    let attr = output.kind.attr();
    Some(match output.kind {
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
        OutputKind::Packages | OutputKind::LegacyPackages => return None,
    })
}

/// Evaluates the packages an output installs; `source` is left for the caller to resolve.
pub fn output_packages(flake_ref: &str, output: &Output) -> Result<Vec<Package>> {
    let Some((target, extract)) = eval_target(output) else {
        return Ok(Vec::new());
    };

    let installable = format!("{}#{}", flake_ref, target);
    crate::log::action(
        "Evaluating nix expressions",
        format!("nix eval {}", installable),
    );

    let output = process::output(
        Command::new("nix")
            .env("NIXPKGS_ALLOW_UNFREE", "1")
            .args(["eval", &installable, "--json", "--impure", "--apply"])
            .arg(PACKAGE_INFO_EXPR.replace("EXTRACT", extract)),
    )
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
    parse_flake_search(&stdout("nix search", cmd, reg)?)
}

/// Parses `nix search --json` output, keyed by attribute path.
pub fn parse_flake_search(json: &[u8]) -> Result<Vec<SearchPackage>> {
    let results: HashMap<String, FlakeSearchPackage> =
        serde_json::from_slice(json).map_err(|e| Error::Parse("nix search", e))?;

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

/// A `nix eval` expression mapping each attribute to its version in `pkgs`, or
/// `"Unknown"`; attributes that are not plain attribute paths are left out.
pub fn versions_expr(pkgs: &str, attrs: &[String]) -> String {
    let mut expr = format!(
        "let pkgs = {}; \
         lib = pkgs.lib; \
         getV = pathStr: let \
           path = lib.splitString \".\" pathStr; \
           pkg = lib.attrByPath path null pkgs; \
           res = if pkg != null then builtins.tryEval (pkg.version or \"Unknown\") else {{ success = false; }}; \
         in if res.success then res.value else \"Unknown\"; \
         in {{ ",
        pkgs
    );
    for attr in attrs.iter().filter(|a| {
        a.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    }) {
        expr.push_str(&format!("\"{}\" = getV \"{}\"; ", attr, attr));
    }
    expr.push('}');
    expr
}

fn eval_versions(
    pkgs: &str,
    attrs: &[String],
    reg: Option<&ChildRegistry>,
) -> HashMap<String, String> {
    if attrs.is_empty() {
        return HashMap::new();
    }
    let mut cmd = Command::new("nix");
    cmd.args([
        "eval",
        "--json",
        "--impure",
        "--expr",
        &versions_expr(pkgs, attrs),
    ]);
    stdout("nix eval", cmd, reg)
        .ok()
        .and_then(|out| serde_json::from_slice(&out).ok())
        .unwrap_or_default()
}

/// Versions of the given attributes at a nixpkgs revision, `"Unknown"` when missing.
pub fn versions_at(
    rev: &str,
    attrs: &[String],
    reg: Option<&ChildRegistry>,
) -> HashMap<String, String> {
    if !rev.chars().all(|c| c.is_ascii_alphanumeric()) {
        return HashMap::new();
    }
    let pkgs = format!(
        "(builtins.getFlake \"github:NixOS/nixpkgs/{}\").legacyPackages.${{builtins.currentSystem}}",
        rev
    );
    eval_versions(&pkgs, attrs, reg)
}

/// Versions of the given attributes in the system `<nixpkgs>`, `"Unknown"` when missing.
pub fn system_versions(attrs: &[String], reg: Option<&ChildRegistry>) -> HashMap<String, String> {
    eval_versions("import <nixpkgs> {}", attrs, reg)
}

pub struct Metadata {
    pub version: String,
    pub description: String,
}

/// Looks `attr` up in the user's channel with `nix-env -qa`.
pub fn metadata(attr: &str) -> Option<Metadata> {
    let output = process::output(Command::new("nix-env").args(["-qa", attr, "--json"]))
        .ok()
        .filter(|o| o.status.success())?;
    parse_metadata(&output.stdout)
}

/// Parses `nix-env -qa <attr> --json` output, using its first entry.
pub fn parse_metadata(json: &[u8]) -> Option<Metadata> {
    let json: serde_json::Value = serde_json::from_slice(json).ok()?;
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

pub fn update_command(flake_ref: &str, inputs: &[String]) -> Command {
    let mut cmd = Command::new("nix");
    cmd.args(["flake", "update"])
        .args(inputs)
        .args(["--flake", flake_ref]);
    cmd
}

pub fn update_inputs(flake_ref: &str, inputs: &[String]) -> Result<()> {
    let mut cmd = update_command(flake_ref, inputs);
    crate::log::action("Updating flake inputs", command_line(&cmd));
    let output = process::output(&mut cmd).map_err(|e| Error::Spawn("nix", e))?;
    if !output.status.success() {
        return Err(Error::Message(format!(
            "nix flake update failed:\n{}",
            stderr_tail(&output.stderr, 20)
        )));
    }
    Ok(())
}

fn command_line(cmd: &Command) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug)]
pub enum CheckError {
    Failed(String),
    NotRun(String),
}

/// Forces each shell's drvPath, which evaluates it without building it.
pub fn check_command(flake_ref: &str, shells: &[ShellTarget]) -> Command {
    let items: Vec<String> = shells
        .iter()
        .map(|t| format!(r#"s."{}"."{}".drvPath"#, t.system, t.name))
        .collect();
    let mut cmd = Command::new("nix");
    cmd.args(["eval", "--json", "--impure", "--no-write-lock-file"])
        .arg(format!("{}#devShells", flake_ref))
        .args(["--apply", &format!("s: [ {} ]", items.join(" "))]);
    cmd
}

pub fn run_check(mut cmd: Command) -> std::result::Result<(), CheckError> {
    let output = process::output(&mut cmd).map_err(|e| CheckError::NotRun(e.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CheckError::Failed(stderr_tail(&output.stderr, 20)))
    }
}

pub fn check_shells(
    flake_ref: &str,
    shells: &[ShellTarget],
) -> std::result::Result<(), CheckError> {
    if shells.is_empty() {
        return Ok(());
    }
    let cmd = check_command(flake_ref, shells);
    crate::log::action("Checking edit", command_line(&cmd));
    run_check(cmd)
}

pub fn stderr_tail(stderr: &[u8], lines: usize) -> String {
    let text = String::from_utf8_lossy(stderr);
    let all: Vec<&str> = text.trim().lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}
