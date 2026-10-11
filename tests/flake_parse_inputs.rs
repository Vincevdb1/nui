mod common;

use common::{flake_src, lock_src};
use nui::flake::parse;

#[test]
fn reads_inputs_with_branch_and_rev_from_the_lock() {
    let inputs = parse::inputs(&flake_src("devshells"), Some(&lock_src("devshells")));

    // `nixpkgs` is only an outputs argument (resolved from the registry), not a declared input.
    let names: Vec<_> = inputs.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["nixpkgs-stable"]);

    let stable = &inputs[0];
    assert_eq!(stable.url, "github:nixos/nixpkgs/nixos-25.11");
    assert_eq!(stable.branch.as_deref(), Some("nixos-25.11"));
    assert_eq!(
        stable.rev.as_deref(),
        Some("107cba9eb4a8d8c9f8e9e61266d78d340867913a")
    );
}

#[test]
fn without_a_lock_inputs_have_no_branch_or_rev() {
    let inputs = parse::inputs(&flake_src("nixos"), None);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "nixpkgs");
    assert_eq!(inputs[0].branch, None);
    assert_eq!(inputs[0].rev, None);
}

#[test]
fn a_malformed_lock_is_ignored() {
    let inputs = parse::inputs(&flake_src("nixos"), Some("{ not json"));
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].url, "github:nixos/nixpkgs/nixos-25.11");
    assert_eq!(inputs[0].rev, None);
}

#[test]
fn nested_input_sets_are_read_and_sorted_by_name() {
    let inputs = parse::inputs(&flake_src("home-manager"), Some(&lock_src("home-manager")));
    let names: Vec<_> = inputs.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["home-manager", "nixpkgs"]);

    let hm = &inputs[0];
    assert_eq!(hm.url, "github:nix-community/home-manager/release-25.11");
    assert_eq!(hm.branch.as_deref(), Some("release-25.11"));
    assert!(hm.rev.is_some(), "home-manager is locked");
    assert!(!hm.is_nixpkgs());
}

#[test]
fn inputs_pinned_to_a_revision_have_no_branch() {
    let inputs = parse::inputs(&flake_src("pinned"), Some(&lock_src("pinned")));
    let hello = inputs.iter().find(|i| i.name == "nixpkgs-hello").unwrap();
    assert_eq!(hello.branch, None);
    assert_eq!(
        hello.rev.as_deref(),
        Some("a4bf06618f0b5ee50f14ed8f0da77d34ecc19160")
    );
    assert_eq!(hello.channel(), "a4bf06618f0b5ee50f14ed8f0da77d34ecc19160");
}

#[test]
fn a_flake_without_inputs_has_none() {
    assert!(parse::inputs(&flake_src("minimal"), None).is_empty());
}

#[test]
fn unparsable_source_gives_no_inputs() {
    assert!(parse::inputs("{ inputs = ", None).is_empty());
}
