mod common;

use common::{Files, assert_parses, flake_files, merged, sources, x86};
use nui::flake::Input;
use nui::flake::edit::{self, EditError};
use std::path::{Path, PathBuf};

const FLAKE: &str = "flake.nix";
const MANAGED: &str = "nui/default.nix";

fn flake() -> &'static Path {
    Path::new(FLAKE)
}

fn changed(changes: &edit::Changes) -> Vec<&Path> {
    changes.iter().map(|(p, _)| p.as_path()).collect()
}

fn packages(files: &Files) -> Vec<String> {
    edit::shell_packages(files, flake(), &x86("default")).unwrap()
}

/// A flake whose default shell sets `packages` to `value`.
fn shell_with(value: &str) -> Files {
    flake_files(&format!(
        r#"{{
  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  outputs =
    {{ self, nixpkgs }}:
    let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      extra = [ pkgs.git ];
    in
    {{
      devShells.x86_64-linux.default = pkgs.mkShell {{
        packages = {value};
      }};
    }};
}}"#
    ))
}

#[test]
fn an_unknown_shell_gets_a_managed_file_and_a_line_to_add() {
    let files = sources("unknown");
    let err = edit::add_package(&files, flake(), &x86("default"), "jq").unwrap_err();
    assert!(matches!(err, EditError::ShellNotFound(_)), "{err:?}");

    let (changes, line) = edit::create_managed(&files, flake(), "default", "jq").unwrap();
    assert_eq!(
        changed(&changes),
        [Path::new(MANAGED)],
        "flake.nix is not edited"
    );
    let managed = &changes[0].1;
    assert_parses(managed);
    assert!(managed.starts_with("# Managed by nui.\n"), "{managed}");
    assert!(managed.contains("with pkgs;\n[\n  jq\n]"), "{managed}");
    assert_eq!(
        line,
        "packages = import ./nui/default.nix { inherit pkgs inputs; };"
    );
}

#[test]
fn after_the_fallback_edits_go_to_the_managed_file() {
    let files = sources("unknown");
    let (changes, _) = edit::create_managed(&files, flake(), "default", "jq").unwrap();
    let files = merged(&files, changes);
    assert_eq!(packages(&files), ["jq"]);

    let changes = edit::add_package(&files, flake(), &x86("default"), "ripgrep").unwrap();
    assert_eq!(changed(&changes), [Path::new(MANAGED)]);
    let files = merged(&files, changes);
    assert_eq!(packages(&files), ["jq", "ripgrep"]);

    let changes = edit::remove_packages(&files, flake(), &x86("default"), &["jq".into()]).unwrap();
    assert_eq!(changed(&changes), [Path::new(MANAGED)]);
    assert_eq!(packages(&merged(&files, changes)), ["ripgrep"]);
}

#[test]
fn a_shell_without_a_list_imports_the_managed_file() {
    let files = shell_with("extra");
    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.jq").unwrap();
    assert_eq!(changed(&changes), [Path::new(FLAKE), Path::new(MANAGED)]);
    let files = merged(&files, changes);
    let flake_src = &files[flake()];
    assert_parses(flake_src);
    assert!(
        flake_src
            .contains("packages = extra ++ import ./nui/default.nix { inherit pkgs inputs; };"),
        "{flake_src}"
    );
    assert!(
        flake_src.contains("inputs@{ self, nixpkgs }:"),
        "{flake_src}"
    );
    assert_eq!(
        packages(&files),
        ["pkgs.git", "pkgs.jq"].map(String::from).to_vec()[1..]
    );

    // Later additions go to the imported file only.
    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.fd").unwrap();
    assert_eq!(changed(&changes), [Path::new(MANAGED)]);
}

#[test]
fn a_value_that_binds_loosely_is_wrapped() {
    let files = shell_with("if true then extra else [ ]");
    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.jq").unwrap();
    let flake_src = &merged(&files, changes)[flake()];
    assert_parses(flake_src);
    assert!(
        flake_src.contains("packages = (if true then extra else [ ]) ++ import ./nui/default.nix"),
        "{flake_src}"
    );
}

#[test]
fn optionals_without_a_plain_list_import_the_managed_file() {
    let files = shell_with("pkgs.lib.optionals true [ pkgs.git ]");
    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.jq").unwrap();
    let files = merged(&files, changes);
    assert_parses(&files[flake()]);
    assert!(files[flake()].contains("[ pkgs.git ] ++ import ./nui/default.nix"));
    assert_eq!(packages(&files), ["pkgs.git", "pkgs.jq"]);
}

#[test]
fn an_outputs_bind_is_passed_as_inputs() {
    let files = flake_files(
        &shell_with("extra")[flake()].replace("{ self, nixpkgs }:", "args@{ self, nixpkgs }:"),
    );
    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.jq").unwrap();
    let flake_src = &merged(&files, changes)[flake()];
    assert!(
        flake_src.contains("import ./nui/default.nix { inherit pkgs; inputs = args; }"),
        "{flake_src}"
    );
    assert!(!flake_src.contains("inputs@"), "{flake_src}");
}

#[test]
fn an_imported_shell_imports_the_managed_file_by_a_relative_path() {
    let mut files = sources("imported");
    files.remove(Path::new("shell.nix"));
    files.insert(
        PathBuf::from("nix/shell.nix"),
        "{ pkgs }:\npkgs.mkShell {\n  packages = pkgs.lib.optionals true [ pkgs.git ];\n}\n".into(),
    );
    let flake_src = files[flake()].replace("./shell.nix", "./nix/shell.nix");
    files.insert(PathBuf::from(FLAKE), flake_src);

    let changes = edit::add_package(&files, flake(), &x86("default"), "pkgs.jq").unwrap();
    assert_eq!(
        changed(&changes),
        [Path::new("nix/shell.nix"), Path::new(MANAGED)]
    );
    let shell = &merged(&files, changes)[Path::new("nix/shell.nix")];
    assert_parses(shell);
    assert!(
        shell.contains("++ import ../nui/default.nix { inherit pkgs; }"),
        "{shell}"
    );
}

#[test]
fn packages_in_the_managed_file_pin_and_unpin() {
    let files = sources("unknown");
    let (changes, _) = edit::create_managed(&files, flake(), "default", "git").unwrap();
    let files = merged(&files, changes);
    let inputs = vec![Input {
        name: "nixpkgs".into(),
        url: "github:nixos/nixpkgs/nixos-25.11".into(),
        branch: None,
        rev: Some("deadbeef".into()),
    }];

    let changes = edit::pin_package(&files, flake(), &x86("default"), "git", &inputs).unwrap();
    assert_eq!(changed(&changes), [Path::new(FLAKE), Path::new(MANAGED)]);
    let files = merged(&files, changes);
    let pinned = "inputs.nixpkgs-git.legacyPackages.${pkgs.stdenv.hostPlatform.system}.git";
    assert_eq!(packages(&files), [pinned]);
    assert!(files[flake()].contains("nixpkgs-git.url"));

    let changes = edit::unpin_package(&files, flake(), &x86("default"), pinned).unwrap();
    let files = merged(&files, changes);
    assert_eq!(
        packages(&files),
        ["git"],
        "a bare name resolves under with pkgs"
    );
    assert!(
        !files[flake()].contains("nixpkgs-git"),
        "{}",
        files[flake()]
    );
}
