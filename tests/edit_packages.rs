mod common;

use common::{assert_parses, flake_src, x86};
use nui::flake::edit::{self, EditError};
use nui::flake::parse;
use std::path::{Path, PathBuf};

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
    let edited = common::add(&flake_src("devshells"), &x86("default"), "pkgs.jq").unwrap();
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
    let edited = common::add(&flake_src("flat-paths"), &x86("default"), "pkgs.ripgrep").unwrap();
    assert_parses(&edited);
    assert_eq!(
        shell_packages(&edited, "default"),
        ["pkgs.git", "pkgs.jq", "pkgs.ripgrep"]
    );
}

#[test]
fn appends_to_the_first_list_of_a_shell_with_several() {
    let edited = common::add(&flake_src("flat-paths"), &x86("native"), "pkgs.cmake").unwrap();
    assert_parses(&edited);
    assert_eq!(
        shell_packages(&edited, "native"),
        ["pkgs.pkg-config", "pkgs.cmake", "pkgs.openssl"]
    );
}

#[test]
fn appends_to_an_inline_list_keeping_it_inline() {
    let edited = common::add(&flake_src("flat-paths"), &x86("native"), "pkgs.cmake").unwrap();
    assert!(
        edited.contains("nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake ];"),
        "{edited}"
    );
}

#[test]
fn fills_an_empty_list() {
    let edited = common::add(&flake_src("minimal"), &x86("default"), "pkgs.hello").unwrap();
    assert_parses(&edited);
    assert!(
        edited.contains("{ packages = [ pkgs.hello ]; }"),
        "{edited}"
    );
}

#[test]
fn creates_a_packages_list_when_the_shell_has_none() {
    let edited = common::add(&flake_src("flat-paths"), &x86("empty"), "pkgs.hello").unwrap();
    assert_parses(&edited);
    assert_eq!(shell_packages(&edited, "empty"), ["pkgs.hello"]);
}

#[test]
fn creates_a_packages_list_with_matching_indentation() {
    let edited = common::add(&flake_src("flat-paths"), &x86("empty"), "pkgs.hello").unwrap();
    assert!(
        edited.contains(
            "devShells.x86_64-linux.empty = pkgs.mkShell {\n        packages = [ pkgs.hello ];\n      };"
        ),
        "{edited}"
    );
}

#[test]
fn adding_to_a_missing_shell_fails() {
    let err = common::add(&flake_src("devshells"), &x86("nope"), "pkgs.jq").unwrap_err();
    assert!(matches!(err, EditError::ShellNotFound(t) if t == x86("nope")));

    let other_system = common::target("aarch64-darwin", "default");
    assert!(common::add(&flake_src("flat-paths"), &other_system, "pkgs.jq").is_err());
}

#[test]
fn removes_a_single_package() {
    let src = flake_src("devshells");
    let edited = common::remove(&src, &x86("default"), &["isort".to_string()]);
    assert_parses(&edited);
    assert_eq!(shell_packages(&edited, "default"), ["python312"]);
    assert_eq!(shell_packages(&edited, "another"), ["hello", "testssl"]);
}

#[test]
fn removes_several_packages_across_lists() {
    let src = flake_src("flat-paths");
    let edited = common::remove(
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
        common::remove(&src, &x86("default"), &["nonexistent".to_string()]),
        src
    );
    assert_eq!(
        common::remove(&src, &x86("missing-shell"), &["isort".to_string()]),
        src
    );
}

#[test]
fn removing_the_last_use_of_an_input_drops_the_input() {
    let src = flake_src("pinned");
    let pinned = "inputs.nixpkgs-hello.legacyPackages.${system}.hello".to_string();
    let edited = common::remove(&src, &x86("default"), &[pinned]);
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
    let edited = common::remove(
        &src,
        &x86("default"),
        &["inputs.nixpkgs-hello.legacyPackages.${system}.hello".to_string()],
    );
    assert_parses(&edited);
    assert!(edited.contains("nixpkgs-hello.url"), "{edited}");
}

#[test]
fn apply_writes_only_changed_files() {
    let dir = common::copy_flake("devshells");
    let path = dir.path().join("flake.nix");
    let fs = &nui::flake::locate::Fs;

    // A no-op edit gives no changes, so a read-only file is never written.
    let writable = std::fs::metadata(&path).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&path, read_only).unwrap();
    let none =
        edit::remove_packages(fs, &path, &x86("default"), &["nonexistent".to_string()]).unwrap();
    assert!(none.is_empty());
    edit::apply(&none).expect("no-op edit must not write");

    std::fs::set_permissions(&path, writable).unwrap();
    let changes = edit::add_package(fs, &path, &x86("default"), "pkgs.jq").unwrap();
    assert_eq!(changes.len(), 1);
    edit::apply(&changes).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(shell_packages(&written, "default").contains(&"pkgs.jq".to_string()));
}

#[test]
fn apply_creates_missing_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nui").join("default.nix");
    edit::apply(&vec![(path.clone(), "[ ]".to_string())]).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ ]");
}

#[test]
fn edits_of_an_unreadable_flake_fail() {
    let missing = std::path::Path::new("missing/flake.nix");
    let err = edit::add_package(&nui::flake::locate::Fs, missing, &x86("default"), "pkgs.jq");
    assert!(matches!(err, Err(EditError::ShellNotFound(_))));
    let err = edit::edit_file(&nui::flake::locate::Fs, missing, |s| s.to_string());
    assert!(matches!(err, Err(EditError::Unreadable(_))));
}

/// Runs an edit on a fixture with all its files; every changed file must parse.
fn edit_fixture(
    fixture: &str,
    edit: impl FnOnce(&common::Files, &Path) -> Result<edit::Changes, EditError>,
) -> (common::Files, Vec<PathBuf>) {
    let files = common::sources(fixture);
    let changes = edit(&files, Path::new("flake.nix")).unwrap();
    let changed: Vec<PathBuf> = changes.iter().map(|(p, _)| p.clone()).collect();
    for (path, src) in &changes {
        assert!(
            rnix::Root::parse(src).errors().is_empty(),
            "{}:\n{src}",
            path.display()
        );
    }
    (common::merged(&files, changes), changed)
}

/// Package items of the located shell, as the view reads them.
fn located_packages(files: &common::Files, system: &str) -> Vec<String> {
    edit::shell_packages(
        files,
        Path::new("flake.nix"),
        &common::target(system, "default"),
    )
    .unwrap()
}

const LAYOUTS: [(&str, &str, &[&str]); 4] = [
    ("flake-utils", "flake.nix", &["pkgs.git", "pkgs.jq"]),
    ("for-all-systems", "flake.nix", &["git", "jq"]),
    ("flake-parts", "flake.nix", &["pkgs.git", "pkgs.jq"]),
    ("imported", "shell.nix", &["pkgs.git", "pkgs.jq"]),
];

#[test]
fn adds_to_shells_in_every_layout() {
    for (fixture, file, before) in LAYOUTS {
        let (files, changed) = edit_fixture(fixture, |s, f| {
            edit::add_package(s, f, &x86("default"), "pkgs.ripgrep")
        });
        assert_eq!(changed, [PathBuf::from(file)], "{fixture}");
        let mut expected: Vec<&str> = before.to_vec();
        expected.push("pkgs.ripgrep");
        assert_eq!(
            located_packages(&files, "x86_64-linux"),
            expected,
            "{fixture}"
        );
    }
}

#[test]
fn removes_from_shells_in_every_layout() {
    for (fixture, file, before) in LAYOUTS {
        let pkg = before[0].to_string();
        let (files, changed) = edit_fixture(fixture, |s, f| {
            edit::remove_packages(s, f, &x86("default"), std::slice::from_ref(&pkg))
        });
        assert_eq!(changed, [PathBuf::from(file)], "{fixture}");
        assert_eq!(
            located_packages(&files, "x86_64-linux"),
            before[1..],
            "{fixture}"
        );
    }
}

#[test]
fn the_view_reads_shells_from_the_locator() {
    let files = common::sources("flake-utils");
    let mut outputs = vec![
        nui::flake::Output {
            kind: nui::flake::OutputKind::DevShell,
            path: "aarch64-darwin.default".to_string(),
            name: None,
            content: None,
            edit_error: None,
        },
        nui::flake::Output {
            kind: nui::flake::OutputKind::DevShell,
            path: "x86_64-linux.missing".to_string(),
            name: None,
            content: None,
            edit_error: None,
        },
    ];
    nui::backend::nix::attach_shell_sources(&files, Path::new("flake.nix"), &mut outputs);
    assert_eq!(
        parse::package_attributes(outputs[0].content.as_deref().unwrap()),
        ["pkgs.git", "pkgs.jq"]
    );
    assert_eq!(outputs[0].edit_error, None);
    assert_eq!(outputs[1].content, None);
    assert!(
        outputs[1]
            .edit_error
            .as_deref()
            .unwrap()
            .contains("could not find devShell")
    );
}

#[test]
fn concatenated_lists_are_all_shown() {
    assert_eq!(
        located_packages(&common::sources("concat"), "x86_64-linux"),
        ["git", "jq", "strace"]
    );
}

#[test]
fn adds_to_the_first_list_of_a_concatenation() {
    let (files, _) = edit_fixture("concat", |s, f| {
        edit::add_package(s, f, &x86("default"), "ripgrep")
    });
    assert_eq!(
        located_packages(&files, "x86_64-linux"),
        ["git", "jq", "ripgrep", "strace"]
    );
    assert!(
        files[Path::new("flake.nix")].contains("            jq\n            ripgrep\n          ]")
    );
}

#[test]
fn removes_from_a_list_inside_optionals() {
    let (files, _) = edit_fixture("concat", |s, f| {
        edit::remove_packages(s, f, &x86("default"), &["strace".to_string()])
    });
    assert_eq!(located_packages(&files, "x86_64-linux"), ["git", "jq"]);
}

#[test]
fn appends_to_the_list_before_optionals() {
    let src = r#"{
  outputs = { self, pkgs }: {
    devShells.x86_64-linux.default = pkgs.mkShell {
      packages = [ pkgs.git ] ++ pkgs.lib.optionals true [ pkgs.strace ];
    };
  };
}"#;
    let edited = common::add(src, &x86("default"), "pkgs.jq").unwrap();
    assert!(
        edited.contains(
            "packages = [ pkgs.git pkgs.jq ] ++ pkgs.lib.optionals true [ pkgs.strace ];"
        ),
        "{edited}"
    );
}
