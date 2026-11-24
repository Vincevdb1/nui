mod common;

use common::{sources, target};
use nui::flake::locate::{LocateError, Location, locate};
use rnix::Root;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn located(
    files: &HashMap<PathBuf, String>,
    system: &str,
    name: &str,
) -> Result<Location, LocateError> {
    locate(files, Path::new("flake.nix"), &target(system, name))
}

/// The text of the located set.
fn set_text(files: &HashMap<PathBuf, String>, location: &Location) -> String {
    let src = &files[&location.file];
    let text = &src[usize::from(location.set.start())..usize::from(location.set.end())];
    assert!(Root::parse(text).errors().is_empty(), "{text}");
    text.to_string()
}

#[test]
fn each_fixture_gives_one_location_in_the_right_file() {
    let cases = [
        ("devshells", "default", "flake.nix", "python312"),
        ("devshells", "another", "flake.nix", "testssl"),
        ("flat-paths", "native", "flake.nix", "pkg-config"),
        ("pinned", "default", "flake.nix", "nixpkgs-hello"),
        ("minimal", "default", "flake.nix", "packages = [ ]"),
        ("flake-utils", "default", "flake.nix", "pkgs.jq"),
        ("for-all-systems", "default", "flake.nix", "jq"),
        ("flake-parts", "default", "flake.nix", "pkgs.jq"),
        ("imported", "default", "shell.nix", "pkgs.jq"),
        ("concat", "default", "flake.nix", "strace"),
    ];
    for (fixture, name, file, contains) in cases {
        let files = sources(fixture);
        let location = located(&files, "x86_64-linux", name)
            .unwrap_or_else(|e| panic!("{fixture}.{name}: {e:?}"));
        assert_eq!(location.file, PathBuf::from(file), "{fixture}");
        let text = set_text(&files, &location);
        assert!(
            text.starts_with('{') && text.contains(contains),
            "{fixture}: {text}"
        );
    }
}

#[test]
fn a_shell_for_any_system_is_found_when_the_system_is_a_variable() {
    for fixture in ["flake-utils", "for-all-systems", "flake-parts"] {
        assert!(
            located(&sources(fixture), "aarch64-darwin", "default").is_ok(),
            "{fixture}"
        );
    }
}

#[test]
fn a_literal_system_must_match() {
    assert_eq!(
        located(&sources("flat-paths"), "aarch64-darwin", "default"),
        Err(LocateError::NotFound)
    );
}

#[test]
fn missing_and_unknown_shells_are_not_found() {
    assert_eq!(
        located(&sources("devshells"), "x86_64-linux", "nope"),
        Err(LocateError::NotFound)
    );
    assert_eq!(
        located(&sources("unknown"), "x86_64-linux", "default"),
        Err(LocateError::NotFound)
    );
    assert_eq!(
        located(&HashMap::new(), "x86_64-linux", "default"),
        Err(LocateError::NotFound)
    );
}

#[test]
fn a_missing_imported_file_is_not_found() {
    let mut files = sources("imported");
    files.remove(Path::new("shell.nix"));
    assert_eq!(
        located(&files, "x86_64-linux", "default"),
        Err(LocateError::NotFound)
    );
}

#[test]
fn two_definitions_are_ambiguous() {
    let src = r#"{
  outputs = { self, nixpkgs }: {
    devShells.x86_64-linux.default = nixpkgs.mkShell { packages = [ ]; };
  } // {
    devShells.x86_64-linux.default = nixpkgs.mkShell { packages = [ ]; };
  };
}"#;
    let files = HashMap::from([(PathBuf::from("flake.nix"), src.to_string())]);
    assert_eq!(
        located(&files, "x86_64-linux", "default"),
        Err(LocateError::Ambiguous(2))
    );
}

#[test]
fn imports_resolve_relative_to_the_importing_file() {
    let flake = r#"{
  outputs = { self, nixpkgs }: {
    devShells = import ./nix/shells.nix { pkgs = nixpkgs; };
  };
}"#;
    let shells = r#"{ pkgs }: { x86_64-linux.default = pkgs.callPackage ./dev.nix { }; }"#;
    let dev = r#"{ mkShell, git }: mkShell { packages = [ git ]; }"#;
    let files = HashMap::from([
        (PathBuf::from("flake.nix"), flake.to_string()),
        (PathBuf::from("nix/shells.nix"), shells.to_string()),
        (PathBuf::from("nix/dev.nix"), dev.to_string()),
    ]);
    let location = located(&files, "x86_64-linux", "default").unwrap();
    assert_eq!(location.file, PathBuf::from("nix/dev.nix"));
    assert_eq!(set_text(&files, &location), "{ packages = [ git ]; }");
}

#[test]
fn a_lambda_parameter_shadows_a_let_binding() {
    let src = r#"{
  outputs = { self, nixpkgs }:
    let shell = nixpkgs.mkShell { packages = [ ]; };
    in { devShells = forAllSystems (shell: { default = shell; }); };
}"#;
    let files = HashMap::from([(PathBuf::from("flake.nix"), src.to_string())]);
    assert_eq!(
        located(&files, "x86_64-linux", "default"),
        Err(LocateError::NotFound)
    );
}
