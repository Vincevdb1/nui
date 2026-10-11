use std::process::Command;

/// The nixpkgs the user's `<nixpkgs>` and NixOS system resolve to.
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub nixpkgs_version: Option<String>,
    pub nixpkgs_path: Option<String>,
    pub nixpkgs_rev: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub nix_search: Option<String>,
    pub nxv: Option<String>,
}

fn trimmed_stdout(cmd: &mut Command) -> Option<String> {
    let output = cmd.output().ok().filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

impl SystemInfo {
    pub fn probe() -> Self {
        let nixpkgs_version = Command::new("nix-instantiate")
            .args([
                "--eval",
                "-E",
                "(import <nixpkgs> {}).lib.version",
                "--json",
            ])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| serde_json::from_slice::<String>(&o.stdout).ok());

        let nixpkgs_path = trimmed_stdout(Command::new("nix").args([
            "eval",
            "--raw",
            "--impure",
            "--expr",
            "toString <nixpkgs>",
        ]));

        let nixpkgs_rev = trimmed_stdout(Command::new("nixos-version").arg("--revision"))
            .filter(|rev| rev.len() >= 7)
            .or_else(|| {
                trimmed_stdout(Command::new("nix").args([
                    "eval",
                    "--raw",
                    "--impure",
                    "--expr",
                    "builtins.substring 0 32 (builtins.baseNameOf (toString <nixpkgs>))",
                ]))
            });

        Self {
            nixpkgs_version,
            nixpkgs_path,
            nixpkgs_rev,
        }
    }
}

impl Tools {
    pub fn probe() -> Self {
        Self {
            nix_search: tool_version("nix-search"),
            nxv: tool_version("nxv"),
        }
    }
}

fn tool_version(cmd: &str) -> Option<String> {
    parse_tool_version(cmd, &trimmed_stdout(Command::new(cmd).arg("--version"))?)
}

/// The version in `<cmd> --version` output: the word after the tool's name, or the last word.
pub fn parse_tool_version(cmd: &str, output: &str) -> Option<String> {
    let first_line = output.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0].eq_ignore_ascii_case(cmd) {
        Some(parts[1].to_string())
    } else {
        parts.last().map(|s| s.to_string())
    }
}
