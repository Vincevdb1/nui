mod common;

use common::{assert_parses, flake_src, lock_src, x86};
use nui::flake::edit::EditError;
use nui::flake::{Input, parse};

const FLAT_REV: &str = "b6018f87da91d19d0ab4cf979885689b469cdd41";
const PINNED_HELLO: &str = "inputs.nixpkgs-hello.legacyPackages.${system}.hello";

fn locked_inputs(fixture: &str) -> Vec<Input> {
    parse::inputs(&flake_src(fixture), Some(&lock_src(fixture)))
}

fn pin_flat(pkg: &str) -> Result<String, EditError> {
    common::pin(
        &flake_src("flat-paths"),
        &x86("default"),
        pkg,
        &locked_inputs("flat-paths"),
    )
}

#[test]
fn pinning_adds_an_input_at_the_locked_revision() {
    let edited = pin_flat("pkgs.git").unwrap();
    assert_parses(&edited);

    let inputs = parse::inputs(&edited, None);
    let pinned = inputs.iter().find(|i| i.name == "nixpkgs-git").unwrap();
    assert_eq!(pinned.url, format!("github:nixos/nixpkgs/{FLAT_REV}"));

    assert!(
        edited.contains("inputs@{ self, nixpkgs, nixpkgs-git }:"),
        "{edited}"
    );
    assert!(
        edited.contains(
            "          inputs.nixpkgs-git.legacyPackages.${pkgs.stdenv.hostPlatform.system}.git\n          pkgs.jq"
        ),
        "the list item is replaced in place:\n{edited}"
    );
}

#[test]
fn bare_and_qualified_names_pin_the_same_item() {
    let src = flake_src("devshells").replace(
        "nixpkgs-stable.url",
        "nixpkgs.url = \"github:nixos/nixpkgs/nixos-25.11\";\n    nixpkgs-stable.url",
    );
    let inputs = vec![Input {
        name: "nixpkgs".to_string(),
        url: "github:nixos/nixpkgs/nixos-25.11".to_string(),
        branch: None,
        rev: Some("deadbeef".to_string()),
    }];
    let edited = common::pin(&src, &x86("default"), "isort", &inputs).unwrap();
    assert_parses(&edited);
    assert!(
        edited.contains(
            "inputs.nixpkgs-isort.legacyPackages.${pkgs.stdenv.hostPlatform.system}.isort"
        ),
        "{edited}"
    );
    assert!(edited.contains("nixpkgs-isort.url = \"github:nixos/nixpkgs/deadbeef\";"));
}

#[test]
fn an_existing_inputs_binding_is_reused() {
    let src = flake_src("pinned");
    let edited = common::pin(&src, &x86("default"), "pkgs.git", &locked_inputs("pinned")).unwrap();
    assert_parses(&edited);
    assert_eq!(edited.matches("inputs@").count(), 1, "{edited}");
}

#[test]
fn pinning_requires_a_locked_source_input() {
    let mut inputs = locked_inputs("flat-paths");
    inputs[0].rev = None;
    let err = common::pin(
        &flake_src("flat-paths"),
        &x86("default"),
        "pkgs.git",
        &inputs,
    );
    assert!(matches!(err, Err(EditError::Unlocked(name)) if name == "nixpkgs"));
}

#[test]
fn pinning_without_a_nixpkgs_input_fails() {
    // devshells takes nixpkgs from the registry, so there is nothing to pin against.
    let err = common::pin(
        &flake_src("devshells"),
        &x86("default"),
        "isort",
        &locked_inputs("devshells"),
    );
    assert!(matches!(err, Err(EditError::NoSourceInput(p)) if p == "isort"));
}

#[test]
fn pinning_an_absent_package_leaves_the_flake_alone() {
    assert!(matches!(
        pin_flat("pkgs.nonexistent"),
        Err(EditError::PackageNotFound(_))
    ));
}

#[test]
fn unpinning_restores_the_package_and_drops_the_input() {
    let edited = common::unpin(&flake_src("pinned"), &x86("default"), PINNED_HELLO).unwrap();
    assert_parses(&edited);
    assert!(!edited.contains("nixpkgs-hello"), "{edited}");
    let inputs: Vec<_> = parse::inputs(&edited, None)
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(inputs, ["nixpkgs"]);
}

#[test]
fn unpinning_writes_a_reference_that_resolves() {
    let edited = common::unpin(&flake_src("pinned"), &x86("default"), PINNED_HELLO).unwrap();
    assert!(
        edited.contains("          pkgs.git\n          pkgs.hello\n"),
        "{edited}"
    );
}

#[test]
fn pin_then_unpin_restores_the_original() {
    let src = flake_src("flat-paths");
    let pinned = pin_flat("pkgs.git").unwrap();
    let unpinned = common::unpin(
        &pinned,
        &x86("default"),
        "inputs.nixpkgs-git.legacyPackages.${pkgs.stdenv.hostPlatform.system}.git",
    )
    .unwrap();
    assert_eq!(unpinned.replace("inputs@", ""), src);
}

#[test]
fn unpinning_rejects_references_that_are_not_pinned() {
    let src = flake_src("pinned");
    assert!(matches!(
        common::unpin(&src, &x86("default"), "pkgs.git"),
        Err(EditError::NotPinned(_))
    ));
    assert!(matches!(
        common::unpin(&src, &x86("default"), "inputs.foo.hello"),
        Err(EditError::InvalidPinnedRef(_))
    ));
    assert!(matches!(
        common::unpin(&src, &x86("missing"), PINNED_HELLO),
        Err(EditError::PackageNotFound(_))
    ));
}

#[test]
fn edit_errors_explain_themselves() {
    let messages = [
        (
            EditError::ShellNotFound(x86("dev")).to_string(),
            "could not find devShell x86_64-linux.dev in flake.nix",
        ),
        (
            EditError::Unlocked("nixpkgs".into()).to_string(),
            "input nixpkgs has no revision in lock file. Run 'nix flake lock' first.",
        ),
        (
            EditError::NotPinned("pkgs.git".into()).to_string(),
            "package pkgs.git is not pinned",
        ),
    ];
    for (actual, expected) in messages {
        assert_eq!(actual, expected);
    }
}

/// A flake whose shell lists one package from a non-nixpkgs input with the given url.
fn flake_with_input(url: &str) -> (String, Vec<Input>) {
    let src = format!(
        r#"{{
  inputs.tools.url = "{url}";
  outputs = {{ self, tools }}: {{
    devShells.x86_64-linux.default = {{ packages = [ tools.packages.x86_64-linux.foo ]; }};
  }};
}}"#
    );
    let inputs = vec![Input {
        name: "tools".to_string(),
        url: url.to_string(),
        branch: None,
        rev: Some("abc123".to_string()),
    }];
    (src, inputs)
}

#[test]
fn pinned_urls_keep_the_input_type() {
    let cases = [
        ("github:owner/tools", "github:owner/tools/abc123"),
        ("github:owner/tools/main", "github:owner/tools/abc123"),
        (
            "git+https://example.org/tools?ref=main",
            "git+https://example.org/tools?rev=abc123",
        ),
    ];
    for (url, expected) in cases {
        let (src, inputs) = flake_with_input(url);
        let edited = common::pin(
            &src,
            &x86("default"),
            "tools.packages.x86_64-linux.foo",
            &inputs,
        )
        .unwrap();
        assert_parses(&edited);
        let pinned = parse::inputs(&edited, None)
            .into_iter()
            .find(|i| i.name == "tools-foo")
            .unwrap_or_else(|| panic!("no pinned input for {url}:\n{edited}"));
        assert_eq!(pinned.url, expected, "url {url}");
    }
}

const HOST: &str = "${pkgs.stdenv.hostPlatform.system}";

fn nixpkgs_input() -> Vec<Input> {
    vec![Input {
        name: "nixpkgs".to_string(),
        url: "github:nixos/nixpkgs/nixos-25.11".to_string(),
        branch: None,
        rev: Some("deadbeef".to_string()),
    }]
}

#[test]
fn pinned_refs_name_the_system_through_pkgs() {
    let edited = pin_flat("pkgs.git").unwrap();
    assert!(
        edited.contains(&format!("inputs.nixpkgs-git.legacyPackages.{HOST}.git")),
        "{edited}"
    );
    assert!(!edited.contains("${system}.git"), "{edited}");
}

#[test]
fn pinning_uses_an_existing_outputs_bind() {
    let files = common::sources("flake-parts");
    let changes = nui::flake::edit::pin_package(
        &files,
        std::path::Path::new("flake.nix"),
        &x86("default"),
        "pkgs.git",
        &nixpkgs_input(),
    )
    .unwrap();
    let edited = &common::merged(&files, changes)[std::path::Path::new("flake.nix")];
    assert_parses(edited);
    let pinned = format!("args.nixpkgs-git.legacyPackages.{HOST}.git");
    assert!(edited.contains(&pinned), "{edited}");
    assert!(!edited.contains("inputs@"), "{edited}");
    assert!(edited.contains("nixpkgs-git.url = \"github:nixos/nixpkgs/deadbeef\";"));

    let unpinned = common::unpin(edited, &x86("default"), &pinned).unwrap();
    assert_parses(&unpinned);
    assert!(unpinned.contains("              pkgs.git\n"), "{unpinned}");
    assert!(!unpinned.contains("nixpkgs-git"), "{unpinned}");
}

#[test]
fn new_and_old_pinned_refs_have_the_same_source() {
    for pinned in [
        PINNED_HELLO.to_string(),
        format!("inputs.nixpkgs-hello.legacyPackages.{HOST}.hello"),
        format!("args.nixpkgs-hello.legacyPackages.{HOST}.hello"),
    ] {
        assert_eq!(
            parse::ref_source(&pinned),
            (Some("nixpkgs-hello"), "hello"),
            "{pinned}"
        );
    }
    assert_eq!(
        parse::ref_source("pkgs.python3Packages.black"),
        (Some("pkgs"), "python3Packages.black")
    );
}

#[test]
fn an_old_system_ref_still_unpins() {
    let edited = common::unpin(&flake_src("pinned"), &x86("default"), PINNED_HELLO).unwrap();
    assert!(edited.contains("pkgs.hello"), "{edited}");
}

#[test]
fn a_new_pinned_ref_is_pinned() {
    let package = nui::flake::Package {
        name: "git".into(),
        description: String::new(),
        version: String::new(),
        is_unfree: false,
        source: None,
        attr: Some(format!("args.nixpkgs-git.legacyPackages.{HOST}.git")),
    };
    assert!(package.is_pinned());
}
