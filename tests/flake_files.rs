mod common;

use nui::flake::files;
use std::path::{Path, PathBuf};

#[test]
fn flake_ref_makes_relative_paths_explicit() {
    let cases = [
        ("", "."),
        (".", "."),
        ("sub", "./sub"),
        ("a/b", "./a/b"),
        ("./sub", "./sub"),
        ("/abs/path", "/abs/path"),
        ("github:owner/repo", "github:owner/repo"),
    ];
    for (path, expected) in cases {
        assert_eq!(files::flake_ref(Path::new(path)), expected, "path {path:?}");
    }
}

#[test]
fn dir_is_the_parent_or_the_current_directory() {
    assert_eq!(files::dir(Path::new("a/b/flake.nix")), Path::new("a/b"));
    assert_eq!(files::dir(Path::new("flake.nix")), Path::new(""));
    assert_eq!(files::dir(Path::new("/")), Path::new("."));
}

#[test]
fn read_lock_finds_the_sibling_lock_file() {
    let lock = files::read_lock(&common::flake_path("devshells")).expect("devshells is locked");
    assert!(lock.contains("\"nixpkgs-stable\""));
    assert_eq!(files::read_lock(&common::flake_path("minimal")), None);
}

/// One test because it changes the process-wide working directory.
#[test]
fn find_lists_flakes_up_to_three_levels_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let touch = |rel: &str| {
        let path = tmp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{ }").unwrap();
    };
    touch("flake.nix");
    touch("a/flake.nix");
    touch("a/b/flake.nix");
    touch("a/b/c/flake.nix"); // depth 4: too deep
    touch("a/not-a-flake.nix");
    for skipped in [".git", "target", "node_modules", "result"] {
        touch(&format!("{skipped}/flake.nix"));
    }

    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let mut found = files::find();
    std::env::set_current_dir(previous).unwrap();

    found.sort();
    assert_eq!(
        found,
        [
            PathBuf::from("a/b/flake.nix"),
            PathBuf::from("a/flake.nix"),
            PathBuf::from("flake.nix"),
        ]
    );
}
