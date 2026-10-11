mod common;

use common::{assert_parses, flake_src};
use nui::flake::{edit, parse};

fn input_names(src: &str) -> Vec<String> {
    parse::inputs(src, None)
        .into_iter()
        .map(|i| i.name)
        .collect()
}

const UNSTABLE: &str = "github:nixos/nixpkgs/nixos-unstable";

#[test]
fn adds_to_the_inputs_set_and_a_single_line_pattern() {
    let edited = edit::add_input(&flake_src("nixos"), "unstable", UNSTABLE);
    assert_parses(&edited);
    assert!(
        edited.contains(
            "    nixpkgs.url = \"github:nixos/nixpkgs/nixos-25.11\";\n    unstable.url = \"github:nixos/nixpkgs/nixos-unstable\";\n  };"
        ),
        "{edited}"
    );
    assert!(
        edited.contains("outputs = { self, nixpkgs, unstable }:"),
        "{edited}"
    );
}

#[test]
fn adds_to_a_multiline_pattern_with_a_trailing_comma() {
    let edited = edit::add_input(&flake_src("devshells"), "unstable", UNSTABLE);
    assert_parses(&edited);
    assert!(
        edited.contains("      nixpkgs-stable,\n      unstable,\n    }:"),
        "{edited}"
    );
}

#[test]
fn adds_to_a_multiline_pattern_without_a_trailing_comma() {
    let src = flake_src("devshells").replace("nixpkgs-stable,\n    }", "nixpkgs-stable\n    }");
    let edited = edit::add_input(&src, "unstable", UNSTABLE);
    assert_parses(&edited);
    assert!(
        edited.contains("      nixpkgs-stable,\n      unstable,\n    }:"),
        "{edited}"
    );
}

#[test]
fn adds_before_an_ellipsis() {
    let edited = edit::add_input(&flake_src("home-manager"), "unstable", UNSTABLE);
    assert_parses(&edited);
    assert!(
        edited.contains("{ nixpkgs, home-manager, unstable, ... }"),
        "{edited}"
    );
}

#[test]
fn dots_in_names_become_dashes() {
    let edited = edit::add_input(&flake_src("nixos"), "nixpkgs.unstable", UNSTABLE);
    assert_parses(&edited);
    assert!(input_names(&edited).contains(&"nixpkgs-unstable".to_string()));
    assert!(edited.contains("nixpkgs-unstable }:"), "{edited}");
}

#[test]
fn adding_an_existing_input_is_a_no_op() {
    let src = flake_src("nixos");
    assert_eq!(edit::add_input(&src, "nixpkgs", UNSTABLE), src);
}

#[test]
fn creates_the_input_when_there_is_no_inputs_block() {
    let edited = edit::add_input(&flake_src("minimal"), "nixpkgs", UNSTABLE);
    assert_parses(&edited);
    assert_eq!(input_names(&edited), ["nixpkgs"]);
    assert!(edited.contains("outputs = { self, nixpkgs }:"), "{edited}");
}

#[test]
fn removes_an_input_and_its_nested_attributes() {
    let edited = edit::remove_input(&flake_src("home-manager"), "home-manager");
    assert_parses(&edited);
    assert_eq!(input_names(&edited), ["nixpkgs"]);
    assert!(
        !edited.contains("follows"),
        "nested `inputs.nixpkgs.follows` is gone:\n{edited}"
    );
    assert!(!edited.contains("release-25.11"), "{edited}");
}

#[test]
fn removes_a_middle_pattern_entry_cleanly() {
    let edited = edit::remove_input(&flake_src("home-manager"), "home-manager");
    assert!(edited.contains("{ nixpkgs, ... }:"), "{edited}");
}

#[test]
fn removes_the_last_pattern_entry_with_its_comma() {
    let edited = edit::remove_input(&flake_src("nixos"), "nixpkgs");
    assert_parses(&edited);
    assert!(input_names(&edited).is_empty());
    assert!(edited.contains("outputs = { self }:"), "{edited}");
}

#[test]
fn removing_an_unknown_input_changes_nothing() {
    let src = flake_src("devshells");
    assert_eq!(edit::remove_input(&src, "nope"), src);
    // A prefix of a real input name is not a match.
    assert_eq!(edit::remove_input(&src, "nixpkgs-stab"), src);
}

#[test]
fn add_then_remove_restores_the_original() {
    for fixture in ["nixos", "flat-paths", "mixed"] {
        let src = flake_src(fixture);
        let added = edit::add_input(&src, "extra", UNSTABLE);
        assert_ne!(added, src, "{fixture}: input was added");
        assert_eq!(edit::remove_input(&added, "extra"), src, "{fixture}");
    }
}
