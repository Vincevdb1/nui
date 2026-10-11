mod common;

use common::{char, flake_app, key, press, shell_app};
use nui::app::update::update;
use nui::app::{Action, Effect, Exit, Focus, Mode, Popup};
use nui::backend::system::{SystemInfo, Tools};
use nui::backend::templates::Template;
use nui::log::LogEntry;
use ratatui::crossterm::event::KeyCode;

#[test]
fn digits_select_panels() {
    let expected = [
        ('1', Focus::Title),
        ('2', Focus::Context),
        ('3', Focus::Inputs),
        ('4', Focus::Outputs),
        ('5', Focus::Log),
    ];
    for (digit, focus) in expected {
        assert_eq!(Focus::from_digit(digit), Some(focus));
    }
    assert_eq!(Focus::from_digit('6'), None);
    assert_eq!(Focus::from_digit('a'), None);
}

#[test]
fn tab_cycles_the_first_four_panels() {
    let mut focus = Focus::Title;
    let mut seen = vec![];
    for _ in 0..4 {
        focus = focus.cycle(true);
        seen.push(focus);
    }
    assert_eq!(
        seen,
        [Focus::Context, Focus::Inputs, Focus::Outputs, Focus::Title]
    );
    assert_eq!(Focus::Title.cycle(false), Focus::Outputs);
    assert_eq!(
        Focus::Log.cycle(true),
        Focus::Title,
        "the log rejoins the cycle"
    );
}

#[test]
fn a_new_shell_app_opens_search_and_probes_the_system() {
    let app = nui::app::App::new(Mode::Shell);
    assert_eq!(app.focus, Focus::Title);
    assert!(matches!(app.popup, Popup::Search(_)));
    assert!(matches!(app.effects.first(), Some(Effect::ProbeSystem)));
    assert!(app.exit.is_none());
}

#[test]
fn q_quits() {
    let mut app = shell_app();
    press(&mut app, char('q'));
    assert!(matches!(app.exit, Some(Exit::Quit)));
}

#[test]
fn question_mark_toggles_help() {
    let mut app = shell_app();
    press(&mut app, char('?'));
    assert!(matches!(app.popup, Popup::Help));
    press(&mut app, char('x'));
    assert!(
        matches!(app.popup, Popup::Help),
        "other keys keep help open"
    );
    press(&mut app, char('?'));
    assert!(matches!(app.popup, Popup::None));
}

#[test]
fn keys_go_to_the_open_popup_not_the_main_view() {
    let mut app = shell_app();
    press(&mut app, char('?'));
    press(&mut app, char('q'));
    assert!(app.exit.is_none(), "q closed help instead of quitting");
    assert!(matches!(app.popup, Popup::None));
}

#[test]
fn t_opens_the_template_picker() {
    let mut app = shell_app();
    press(&mut app, char('t'));
    assert!(matches!(app.popup, Popup::Templates(_)));
    press(&mut app, key(KeyCode::Esc));
    assert!(matches!(app.popup, Popup::None));
}

#[test]
fn switching_to_shell_mode_clears_the_flake_and_opens_search() {
    let mut app = flake_app("devshells");
    press(&mut app, char('m'));
    assert_eq!(app.mode, Mode::Shell);
    assert!(app.flake.outputs.is_empty());
    assert!(matches!(app.popup, Popup::Search(_)));
    assert!(matches!(app.effects.last(), Some(Effect::CancelSearch(_))));
}

#[test]
fn switching_to_flake_mode_focuses_the_title() {
    let mut app = shell_app();
    press(&mut app, char('m'));
    assert_eq!(app.mode, Mode::Flake);
    assert_eq!(app.focus, Focus::Title);
}

#[test]
fn a_probed_system_starts_an_nxv_update_only_when_nxv_exists() {
    let mut app = shell_app();
    let info = SystemInfo {
        nixpkgs_version: Some("25.11".into()),
        nixpkgs_path: None,
        nixpkgs_rev: Some("abc".into()),
    };
    update(
        &mut app,
        Action::SystemProbed(info.clone(), Tools::default()),
    );
    assert!(app.effects.is_empty());
    assert_eq!(app.system_version(), Some("25.11"));
    assert_eq!(app.system_rev(), Some("abc"));

    let tools = Tools {
        nix_search: None,
        nxv: Some("0.7.3".into()),
    };
    update(&mut app, Action::SystemProbed(info, tools));
    assert!(matches!(app.effects.as_slice(), [Effect::UpdateNxv]));
}

#[test]
fn background_results_update_the_app() {
    let mut app = shell_app();
    update(
        &mut app,
        Action::TemplatesListed(vec![Template {
            name: "rust.nix".into(),
            description: "Rust".into(),
        }]),
    );
    assert_eq!(app.templates.len(), 1);

    update(&mut app, Action::NxvProgress(Some("[█    ] 10%".into())));
    assert_eq!(app.nxv_progress.as_deref(), Some("[█    ] 10%"));
    update(&mut app, Action::NxvProgress(None));
    assert_eq!(app.nxv_progress, None);

    update(&mut app, Action::Log(LogEntry::Info("hi".into())));
    assert_eq!(app.log.entries.len(), 1);

    update(
        &mut app,
        Action::LatestVersion {
            attr: "hello".into(),
            version: "2.13".into(),
        },
    );
    assert_eq!(
        app.flake.updates.get("hello").map(String::as_str),
        Some("2.13")
    );
}

#[test]
fn a_flake_change_reloads_the_selected_file() {
    let mut app = flake_app("devshells");
    update(&mut app, Action::FlakeChanged);
    assert!(app.flake.loading);
    match app.effects.as_slice() {
        [Effect::LoadFlake(path)] => assert_eq!(path, &common::flake_path("devshells")),
        _ => panic!("expected a single LoadFlake effect"),
    }
}
