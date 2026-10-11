mod common;

use common::{assert_parses, flake_src, x86};
use nui::flake::edit::{self, EditError};
use nui::flake::parse;

/// Package list items of a devShell after an edit, as written in the source.
fn shell_packages(src: &str, name: &str) -> Vec<String> {
    let path = format!("x86_64-linux.{name}");
    let output = parse::outputs(src)
        .into_iter()
        .find(|o| o.path == path)
        .unwrap_or_else(|| panic!("no devShell {path} in:\n{src}"));
    parse::package_attributes(output.content.as_deref().unwrap())
}

#[test]
fn appends_to_a_multiline_list_keeping_its_indentation() {
    let edited = edit::add_package(&flake_src("devshells"), &x86("default"), "pkgs.jq").unwrap();
    assert_parses(&edited);
    assert!(
        edited.contains("            isort\n            pkgs.jq\n          ];"),
        "{edited}"
    );
    assert_eq!(
        shell_packages(&edited, "default"),
        ["python312", "isort", "pkgs.jq"]
    );
    assert_eq!(
        shell_packages(&edited, "another"),
        ["hello", "testssl"],
        "other shells are untouched"
    );
}

#[test]
fn finds_shells_on_dotted_paths_with_a_literal_system() {
    let edited =
        edit::add_package(&flake_src("flat-paths"), &x86("default"), "pkgs.ripgrep").unwrap();
    assert_parses(&edited);
    assert_eq!(
        shell_packages(&edited, "default"),
        ["pkgs.git", "pkgs.jq", "pkgs.ripgrep"]
    );
}

#[test]
fn appends_to_the_first_list_of_a_shell_with_several() {
    let edited = edit::add_package(&flake_src("flat-paths"), &x86("native"), "pkgs.cmake").unwrap();
    assert_parses(&edited);
    assert_eq!(
        shell_packages(&edited, "native"),
        ["pkgs.pkg-config", "pkgs.cmake", "pkgs.openssl"]
    );
}

#[test]
#[ignore = "known bug: appending to an inline list drops the space before `]`"]
fn appends_to_an_inline_list_keeping_it_inline() {
    let edited = edit::add_package(&flake_src("flat-paths"), &x86("native"), "pkgs.cmake").unwrap();
    assert!(
        edited.contains("nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake ];"),
        "{edited}"
    );
}

#[test]
fn fills_an_empty_list() {
    let edited = edit::add_package(&flake_src("minimal"), &x86("default"), "pkgs.hello").unwrap();
    assert_parses(&edited);
    assert!(
        edited.contains("{ packages = [ pkgs.hello ]; }"),
        "{edited}"
    );
}

#[test]
fn creates_a_packages_list_when_the_shell_has_none() {
    let edited = edit::add_package(&flake_src("flat-paths"), &x86("empty"), "pkgs.hello").unwrap();
    assert_parses(&edited);
    assert_eq!(shell_packages(&edited, "empty"), ["pkgs.hello"]);
}

#[test]
#[ignore = "known bug: a packages list added to an empty `mkShell { }` is mis-indented"]
fn creates_a_packages_list_with_matching_indentation() {
    let edited = edit::add_package(&flake_src("flat-paths"), &x86("empty"), "pkgs.hello").unwrap();
    assert!(
        edited.contains(
            "devShells.x86_64-linux.empty = pkgs.mkShell {\n        packages = [ pkgs.hello ];\n      };"
        ),
        "{edited}"
    );
}

#[test]
fn adding_to_a_missing_shell_fails() {
    let err = edit::add_package(&flake_src("devshells"), &x86("nope"), "pkgs.jq").unwrap_err();
    assert!(matches!(err, EditError::ShellNotFound(t) if t == x86("nope")));

    let other_system = common::target("aarch64-darwin", "default");
    assert!(edit::add_package(&flake_src("flat-paths"), &other_system, "pkgs.jq").is_err());
}

#[test]
fn removes_a_single_package() {
    let src = flake_src("devshells");
    let edited = edit::remove_packages(&src, &x86("default"), &["isort".to_string()]);
    assert_parses(&edited);
    assert_eq!(shell_packages(&edited, "default"), ["python312"]);
    assert_eq!(shell_packages(&edited, "another"), ["hello", "testssl"]);
}

#[test]
fn removes_several_packages_across_lists() {
    let src = flake_src("flat-paths");
    let edited = edit::remove_packages(
        &src,
        &x86("native"),
        &["pkgs.pkg-config".to_string(), "pkgs.openssl".to_string()],
    );
    assert_parses(&edited);
    assert!(shell_packages(&edited, "native").is_empty());
    assert!(edited.contains("nativeBuildInputs = [ ];"), "{edited}");
}

#[test]
fn removing_an_absent_package_changes_nothing() {
    let src = flake_src("devshells");
    assert_eq!(
        edit::remove_packages(&src, &x86("default"), &["nonexistent".to_string()]),
        src
    );
    assert_eq!(
        edit::remove_packages(&src, &x86("missing-shell"), &["isort".to_string()]),
        src
    );
}

#[test]
fn removing_the_last_use_of_an_input_drops_the_input() {
    let src = flake_src("pinned");
    let pinned = "inputs.nixpkgs-hello.legacyPackages.${system}.hello".to_string();
    let edited = edit::remove_packages(&src, &x86("default"), &[pinned]);
    assert_parses(&edited);
    assert_eq!(shell_packages(&edited, "default"), ["pkgs.git"]);
    let inputs: Vec<_> = parse::inputs(&edited, None)
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(inputs, ["nixpkgs"]);
    assert!(!edited.contains("nixpkgs-hello"), "{edited}");
}

#[test]
fn an_input_still_in_use_is_kept() {
    // A second package from the same input keeps the input referenced.
    let src = flake_src("pinned").replace(
        "packages = [\n          pkgs.git\n",
        "packages = [\n          pkgs.git\n          inputs.nixpkgs-hello.legacyPackages.${system}.cowsay\n",
    );
    let edited = edit::remove_packages(
        &src,
        &x86("default"),
        &["inputs.nixpkgs-hello.legacyPackages.${system}.hello".to_string()],
    );
    assert_parses(&edited);
    assert!(edited.contains("nixpkgs-hello.url"), "{edited}");
}

#[test]
fn rewrite_only_writes_when_the_edit_changes_something() {
    let dir = common::copy_flake("devshells");
    let path = dir.path().join("flake.nix");

    // Make the file read-only: an unchanged edit must not try to write it.
    let writable = std::fs::metadata(&path).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&path, read_only).unwrap();
    edit::rewrite(&path, |src| Ok(src.to_string())).expect("no-op edit must not write");

    std::fs::set_permissions(&path, writable).unwrap();
    edit::rewrite(&path, |src| {
        edit::add_package(src, &x86("default"), "pkgs.jq")
    })
    .unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(shell_packages(&written, "default").contains(&"pkgs.jq".to_string()));
}

#[test]
fn rewrite_propagates_edit_and_io_errors() {
    let dir = common::copy_flake("devshells");
    let path = dir.path().join("flake.nix");
    let before = std::fs::read_to_string(&path).unwrap();

    let err = edit::rewrite(&path, |src| edit::add_package(src, &x86("nope"), "pkgs.jq"));
    assert!(matches!(err, Err(EditError::ShellNotFound(_))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

    let missing = dir.path().join("missing.nix");
    assert!(matches!(
        edit::rewrite(&missing, |src| Ok(src.to_string())),
        Err(EditError::Io(_))
    ));
}
