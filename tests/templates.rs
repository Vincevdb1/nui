//! Template storage tests. They point `XDG_CONFIG_HOME` at a temporary directory, which is
//! process-wide, so they live in their own test binary and share one directory.

mod common;

use nui::backend::templates;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Sets up the config directory once and serialises the tests that use it.
fn config() -> (MutexGuard<'static, ()>, PathBuf) {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: only tests in this binary read the environment, and they hold `LOCK`.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
        dir
    });
    let templates = dir.path().join("nui").join("templates");
    let _ = std::fs::remove_dir_all(&templates);
    (guard, templates)
}

#[test]
fn templates_live_in_the_nui_config_directory() {
    let (_guard, expected) = config();
    assert_eq!(templates::dir(), Some(expected.clone()));
    assert_eq!(templates::path("rust.nix"), Some(expected.join("rust.nix")));

    assert!(
        templates::list().is_empty(),
        "missing directory lists nothing"
    );
    templates::ensure_dir();
    assert!(expected.is_dir());
}

#[test]
fn saved_shells_round_trip_through_list_and_packages() {
    let (_guard, dir) = config();
    templates::ensure_dir();

    let packages = ["git".to_string(), "ripgrep".to_string()];
    let file = templates::save_shell("tools", "My tools", &packages).unwrap();
    assert_eq!(file, "tools.nix");
    assert_eq!(
        templates::save_shell("other.nix", "", &[]).unwrap(),
        "other.nix",
        "an existing .nix extension is kept"
    );

    let content = std::fs::read_to_string(dir.join("tools.nix")).unwrap();
    common::assert_parses(&content);
    assert_eq!(templates::packages(&content), packages);

    let mut listed: Vec<_> = templates::list()
        .into_iter()
        .map(|t| (t.name, t.description))
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            ("other.nix".to_string(), String::new()),
            ("tools.nix".to_string(), "My tools".to_string()),
        ]
    );
}

#[test]
fn templates_without_a_description_say_so() {
    let (_guard, dir) = config();
    templates::ensure_dir();
    std::fs::write(dir.join("bare.nix"), "{ outputs = _: { }; }").unwrap();
    std::fs::create_dir(dir.join("subdir")).unwrap();

    let listed = templates::list();
    assert_eq!(listed.len(), 1, "directories are not templates");
    assert_eq!(listed[0].name, "bare.nix");
    assert_eq!(listed[0].description, "No description");
}

#[test]
fn every_fixture_flake_can_serve_as_a_template() {
    let (_guard, dir) = config();
    templates::ensure_dir();
    std::fs::copy(common::flake_path("devshells"), dir.join("python.nix")).unwrap();

    let listed = templates::list();
    assert_eq!(listed[0].description, "This is a test flake");
    let content = std::fs::read_to_string(dir.join("python.nix")).unwrap();
    assert_eq!(
        templates::packages(&content),
        Vec::<String>::new(),
        "`with pkgs;` lists are skipped by the simple template scanner"
    );
}
