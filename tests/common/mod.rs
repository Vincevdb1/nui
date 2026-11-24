//! Helpers shared by the integration tests: fixture loading and app construction.
#![allow(dead_code)]

use nui::app::{Action, App, Mode, Popup, update::update};
use nui::flake::{ShellTarget, parse};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path of `test_flakes/<name>/flake.nix`.
pub fn flake_path(name: &str) -> PathBuf {
    root().join("test_flakes").join(name).join("flake.nix")
}

pub fn flake_src(name: &str) -> String {
    std::fs::read_to_string(flake_path(name)).expect("fixture flake.nix")
}

pub fn lock_src(name: &str) -> String {
    std::fs::read_to_string(root().join("test_flakes").join(name).join("flake.lock"))
        .expect("fixture flake.lock")
}

/// Contents of `tests/fixtures/<file>`.
pub fn fixture(file: &str) -> String {
    std::fs::read_to_string(root().join("tests").join("fixtures").join(file)).expect("fixture")
}

/// Copies a fixture flake into a fresh temporary directory.
pub fn copy_flake(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = root().join("test_flakes").join(name);
    for file in ["flake.nix", "flake.lock"] {
        if src.join(file).exists() {
            std::fs::copy(src.join(file), dir.path().join(file)).expect("copy fixture");
        }
    }
    dir
}

pub fn target(system: &str, name: &str) -> ShellTarget {
    ShellTarget {
        system: system.to_string(),
        name: name.to_string(),
    }
}

pub fn x86(name: &str) -> ShellTarget {
    target("x86_64-linux", name)
}

/// Asserts that `src` is valid Nix syntax, printing it when it isn't.
pub fn assert_parses(src: &str) {
    let errors = rnix::Root::parse(src).errors().to_vec();
    assert!(
        errors.is_empty(),
        "edit produced invalid nix {errors:?}:\n{src}"
    );
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub fn char(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

pub fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

pub fn alt_enter() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
}

pub fn press(app: &mut App, key: KeyEvent) {
    update(app, Action::Key(key));
}

pub fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, char(c));
    }
}

/// A shell-mode app with no popup open and no pending effects.
pub fn shell_app() -> App {
    let mut app = App::new(Mode::Shell);
    app.popup = Popup::None;
    app.effects.clear();
    app
}

/// A flake-mode app showing a fixture flake, with outputs read from its source so no
/// `nix` evaluation is needed.
pub fn flake_app(name: &str) -> App {
    let mut app = shell_app();
    app.mode = Mode::Flake;
    let path = flake_path(name);
    let src = flake_src(name);
    let lock = std::fs::read_to_string(path.with_file_name("flake.lock")).ok();
    app.flake.files = vec![path];
    app.flake.inputs = parse::inputs(&src, lock.as_deref());
    app.flake.outputs = parse::outputs(&src);
    app
}

/// Every `.nix` file of a fixture flake, keyed by its path relative to the flake.
pub fn sources(name: &str) -> std::collections::HashMap<PathBuf, String> {
    let dir = root().join("test_flakes").join(name);
    walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "nix"))
        .map(|e| {
            let rel = e.path().strip_prefix(&dir).unwrap().to_path_buf();
            (rel, std::fs::read_to_string(e.path()).unwrap())
        })
        .collect()
}

pub type Files = std::collections::HashMap<PathBuf, String>;

pub fn flake_files(src: &str) -> Files {
    Files::from([(PathBuf::from("flake.nix"), src.to_string())])
}

/// `files` with the changes of an edit applied.
pub fn merged(files: &Files, changes: nui::flake::edit::Changes) -> Files {
    let mut files = files.clone();
    files.extend(changes);
    files
}

fn on_flake(
    src: &str,
    edit: impl FnOnce(
        &Files,
        &std::path::Path,
    ) -> Result<nui::flake::edit::Changes, nui::flake::edit::EditError>,
) -> Result<String, nui::flake::edit::EditError> {
    let files = flake_files(src);
    let changes = edit(&files, std::path::Path::new("flake.nix"))?;
    Ok(merged(&files, changes)
        .remove(std::path::Path::new("flake.nix"))
        .unwrap())
}

pub fn add(
    src: &str,
    target: &ShellTarget,
    pkg: &str,
) -> Result<String, nui::flake::edit::EditError> {
    on_flake(src, |s, f| nui::flake::edit::add_package(s, f, target, pkg))
}

pub fn remove(src: &str, target: &ShellTarget, pkgs: &[String]) -> String {
    on_flake(src, |s, f| {
        nui::flake::edit::remove_packages(s, f, target, pkgs)
    })
    .unwrap()
}

pub fn pin(
    src: &str,
    target: &ShellTarget,
    pkg: &str,
    inputs: &[nui::flake::Input],
) -> Result<String, nui::flake::edit::EditError> {
    on_flake(src, |s, f| {
        nui::flake::edit::pin_package(s, f, target, pkg, inputs)
    })
}

pub fn unpin(
    src: &str,
    target: &ShellTarget,
    pkg: &str,
) -> Result<String, nui::flake::edit::EditError> {
    on_flake(src, |s, f| {
        nui::flake::edit::unpin_package(s, f, target, pkg)
    })
}
