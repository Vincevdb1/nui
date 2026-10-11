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
