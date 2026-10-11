mod common;

use common::{alt_enter, char, ctrl, flake_app, key, press, shell_app, type_text, x86};
use nui::app::effect::FlakeEdit;
use nui::app::update::update;
use nui::app::{Action, App, Effect, Generation, Mode, Popup};
use nui::backend::nxv::VersionInfo;
use nui::backend::search::{ChannelVersion, Scope, SearchResult};
use nui::backend::system::SystemInfo;
use nui::features::search::{self, Search, Stage};
use nui::flake::Input;
use nui::pkgref::Source;
use ratatui::crossterm::event::KeyCode;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(350);

fn result(name: &str, versions: &[(&str, &str)], hash: Option<&str>) -> SearchResult {
    SearchResult {
        name: name.to_string(),
        description: format!("{name} description"),
        versions: versions
            .iter()
            .map(|(channel, version)| ChannelVersion {
                version: version.to_string(),
                channel: channel.to_string(),
                locked_version: None,
            })
            .collect(),
        platforms: vec![],
        is_unfree: false,
        source_input: None,
        hash: hash.map(str::to_string),
    }
}

fn system(rev: Option<&str>) -> SystemInfo {
    SystemInfo {
        nixpkgs_version: Some("25.11.20260630.b6018f8".into()),
        nixpkgs_path: None,
        nixpkgs_rev: rev.map(str::to_string),
    }
}

fn search(app: &App) -> &Search {
    match &app.popup {
        Popup::Search(s) => s,
        _ => panic!("the search popup is not open"),
    }
}

fn open(mut app: App) -> App {
    search::open(&mut app);
    app.effects.clear();
    app
}

/// Opens search with results already delivered for the current generation.
fn with_results(mut app: App, results: Vec<SearchResult>) -> App {
    app = open(app);
    let generation = app.search_generation;
    update(&mut app, Action::SearchFinished(generation, Ok(results)));
    app.effects.clear();
    app
}

fn shell_with_system(rev: Option<&str>) -> App {
    let mut app = shell_app();
    app.system = Some(system(rev));
    app
}

#[test]
fn typing_edits_the_query() {
    let mut app = open(shell_app());
    type_text(&mut app, "ripx");
    press(&mut app, key(KeyCode::Backspace));
    assert_eq!(search(&app).query, "rip");
}

#[test]
fn searches_start_only_after_typing_pauses() {
    let mut app = open(shell_with_system(Some("abc")));
    type_text(&mut app, "rg");
    update(&mut app, Action::Tick);
    assert!(app.effects.is_empty(), "no search while still typing");

    std::thread::sleep(DEBOUNCE);
    update(&mut app, Action::Tick);
    match app.effects.as_slice() {
        [
            Effect::Search {
                generation,
                request,
            },
        ] => {
            assert_eq!(*generation, app.search_generation);
            assert_eq!(request.query, "rg");
        }
        _ => panic!("expected one Search effect"),
    }
    assert!(search(&app).searching);
    assert_eq!(search(&app).channels, ["25.11.20260630.b6018f8", "nxv"]);

    update(&mut app, Action::Tick);
    assert_eq!(app.effects.len(), 1, "the same query isn't searched twice");
}

#[test]
fn shell_searches_wait_for_the_system_probe() {
    let mut app = open(shell_app());
    type_text(&mut app, "rg");
    std::thread::sleep(DEBOUNCE);
    update(&mut app, Action::Tick);
    assert!(app.effects.is_empty());

    app.system = Some(system(None));
    update(&mut app, Action::Tick);
    assert!(matches!(app.effects.as_slice(), [Effect::Search { .. }]));
}

#[test]
fn clearing_the_query_clears_results() {
    let mut app = with_results(shell_with_system(None), vec![result("rg", &[], None)]);
    type_text(&mut app, "r");
    std::thread::sleep(DEBOUNCE);
    update(&mut app, Action::Tick);
    press(&mut app, key(KeyCode::Backspace));
    std::thread::sleep(DEBOUNCE);
    update(&mut app, Action::Tick);
    assert!(search(&app).results.is_empty());
    assert_eq!(search(&app).list.selected(), None);
}

#[test]
fn results_select_the_first_entry() {
    let app = with_results(
        shell_app(),
        vec![result("rg", &[], None), result("rga", &[], None)],
    );
    let s = search(&app);
    assert_eq!(s.results.len(), 2);
    assert_eq!(s.list.selected(), Some(0));
    assert!(!s.searching);
}

#[test]
fn results_of_a_superseded_search_are_dropped() {
    let mut app = open(shell_app());
    let stale = app.search_generation;
    search::open(&mut app); // a new search supersedes the old one
    update(
        &mut app,
        Action::SearchFinished(stale, Ok(vec![result("rg", &[], None)])),
    );
    assert!(search(&app).results.is_empty());
}

#[test]
fn search_errors_are_shown() {
    let mut app = open(shell_app());
    let generation = app.search_generation;
    update(
        &mut app,
        Action::SearchFinished(generation, Err("offline".into())),
    );
    assert_eq!(search(&app).error.as_deref(), Some("offline"));
}

#[test]
fn flake_results_ask_for_locked_versions() {
    let app = flake_app("pinned");
    let mut app = open(app);
    let generation = app.search_generation;
    update(
        &mut app,
        Action::SearchFinished(generation, Ok(vec![result("hello", &[], None)])),
    );
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::LockedVersions { results, inputs, .. }] if results.len() == 1 && inputs.len() == 2
    ));
}

#[test]
fn ctrl_j_and_arrows_move_the_selection() {
    let mut app = with_results(
        shell_app(),
        vec![result("a", &[], None), result("b", &[], None)],
    );
    press(&mut app, ctrl('j'));
    assert_eq!(search(&app).list.selected(), Some(1));
    press(&mut app, key(KeyCode::Down));
    assert_eq!(search(&app).list.selected(), Some(0));
    press(&mut app, key(KeyCode::Up));
    assert_eq!(search(&app).list.selected(), Some(1));
    assert_eq!(search(&app).query, "", "ctrl+j is not typed");
}

#[test]
fn tab_toggles_details_and_esc_closes() {
    let mut app = with_results(shell_app(), vec![result("rg", &[], None)]);
    press(&mut app, key(KeyCode::Tab));
    assert!(matches!(search(&app).stage, Stage::Details));
    press(&mut app, char('x'));
    assert!(matches!(search(&app).stage, Stage::Details));
    press(&mut app, key(KeyCode::Tab));
    assert!(matches!(search(&app).stage, Stage::Results));
    press(&mut app, key(KeyCode::Esc));
    assert!(matches!(app.popup, Popup::None));
    assert!(matches!(app.effects.last(), Some(Effect::CancelSearch(_))));
}

#[test]
fn enter_in_shell_mode_adds_from_the_system_nixpkgs() {
    let mut app = with_results(
        shell_with_system(Some("sysrev")),
        vec![result(
            "rg",
            &[("nxv", "15.2"), ("system", "15.1")],
            Some("nxvhash"),
        )],
    );
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    let added = &app.shell.packages[0].reference;
    assert_eq!(added.source, Source::System("sysrev".into()));
    assert_eq!(added.version.as_deref(), Some("15.1"));
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::ShellMetadata(r) if r == added))
    );
}

#[test]
fn enter_without_a_system_revision_uses_the_result_hash_or_registry() {
    let mut app = with_results(
        shell_with_system(None),
        vec![result("rg", &[], Some("hash"))],
    );
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(
        app.shell.packages[0].reference.source,
        Source::Nixpkgs("hash".into())
    );

    let mut app = with_results(shell_with_system(None), vec![result("rg", &[], None)]);
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(app.shell.packages[0].reference.source, Source::Registry);
    assert_eq!(
        app.shell.packages[0].reference.version.as_deref(),
        Some("Unknown")
    );
}

#[test]
fn adding_the_same_package_twice_keeps_one() {
    let mut app = shell_with_system(Some("sysrev"));
    for _ in 0..2 {
        app = with_results(app, vec![result("rg", &[("system", "1")], None)]);
        press(&mut app, key(KeyCode::Enter));
    }
    assert_eq!(app.shell.packages.len(), 1);
}

#[test]
fn enter_in_flake_mode_adds_to_the_selected_devshell() {
    let mut app = with_results(flake_app("devshells"), vec![result("jq", &[], None)]);
    press(&mut app, key(KeyCode::Enter));
    let edit = app.effects.iter().find_map(|e| match e {
        Effect::Edit {
            edit: FlakeEdit::AddPackage { target, package },
            ..
        } => Some((target.clone(), package.clone())),
        _ => None,
    });
    assert_eq!(edit, Some((x86("default"), "jq".to_string())));
}

#[test]
fn enter_without_results_does_nothing() {
    let mut app = with_results(shell_app(), vec![]);
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::Search(_)));
    assert!(app.shell.packages.is_empty());
}

#[test]
fn locked_and_latest_versions_update_matching_columns() {
    let mut app = with_results(
        flake_app("pinned"),
        vec![result(
            "hello",
            &[("nixos-25.11", "2.12"), ("tools", "0.1")],
            None,
        )],
    );
    let generation = app.search_generation;
    update(
        &mut app,
        Action::LockedVersion {
            generation,
            attr: "hello".into(),
            channel: "nixos-25.11".into(),
            version: "2.10".into(),
        },
    );
    update(
        &mut app,
        Action::LatestVersion {
            attr: "hello".into(),
            version: "2.13".into(),
        },
    );
    let versions = &search(&app).results[0].versions;
    assert_eq!(versions[0].locked_version.as_deref(), Some("2.10"));
    assert_eq!(
        versions[0].version, "2.13",
        "nixpkgs columns get the latest version"
    );
    assert_eq!(versions[1].version, "0.1", "other inputs are untouched");
    assert_eq!(versions[1].locked_version, None);
}

#[test]
fn stale_locked_versions_are_ignored() {
    let mut app = with_results(
        flake_app("pinned"),
        vec![result("hello", &[("nixos-25.11", "1")], None)],
    );
    update(
        &mut app,
        Action::LockedVersion {
            generation: Generation::default(),
            attr: "hello".into(),
            channel: "nixos-25.11".into(),
            version: "0.9".into(),
        },
    );
    assert_eq!(search(&app).results[0].versions[0].locked_version, None);
}

fn version(v: &str, hash: &str) -> VersionInfo {
    VersionInfo {
        version: v.into(),
        hash: hash.into(),
        date: "2026-01-01".into(),
        is_unfree: false,
        is_system: false,
    }
}

/// Opens the version picker for the selected result and delivers its history.
fn pick_versions(mut app: App, history: Vec<VersionInfo>) -> App {
    press(&mut app, alt_enter());
    assert!(matches!(app.effects.last(), Some(Effect::FetchVersions(attr)) if attr == "rg"));
    update(&mut app, Action::VersionsLoaded("rg".into(), Ok(history)));
    app
}

#[test]
fn shell_version_history_marks_the_system_revision() {
    let app = with_results(
        shell_with_system(Some("sysrev")),
        vec![result("rg", &[("nxv", "15.2")], None)],
    );
    let app = pick_versions(app, vec![version("15.2", "a"), version("14.1", "sysrev")]);
    let Stage::Versions(picker) = &search(&app).stage else {
        panic!("expected the version picker");
    };
    assert!(!picker.loading);
    let system: Vec<_> = picker
        .items
        .iter()
        .filter_map(|i| match i {
            nui::features::search::versions::VersionItem::Version(v) if v.is_system => {
                Some(v.version.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(system, ["14.1"]);
}

#[test]
fn a_missing_system_revision_is_added_to_the_history() {
    let app = with_results(
        shell_with_system(Some("sysrev")),
        vec![result("rg", &[("system", "15.0")], None)],
    );
    let app = pick_versions(app, vec![version("15.2", "a")]);
    let Stage::Versions(picker) = &search(&app).stage else {
        panic!("expected the version picker");
    };
    assert_eq!(picker.items.len(), 2);
}

#[test]
fn picking_a_version_in_shell_mode_adds_it() {
    let app = with_results(shell_with_system(None), vec![result("rg", &[], None)]);
    let mut app = pick_versions(
        app,
        vec![version("15.2", "newhash"), version("14.1", "old")],
    );
    press(&mut app, char('j'));
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    let reference = &app.shell.packages[0].reference;
    assert_eq!(reference.source, Source::Nixpkgs("old".into()));
    assert_eq!(reference.version.as_deref(), Some("14.1"));
}

#[test]
fn esc_in_the_version_picker_returns_to_results() {
    let app = with_results(shell_with_system(None), vec![result("rg", &[], None)]);
    let mut app = pick_versions(app, vec![version("1", "a")]);
    press(&mut app, key(KeyCode::Esc));
    assert!(matches!(search(&app).stage, Stage::Results));
}

#[test]
fn version_errors_are_shown_in_the_picker() {
    let mut app = with_results(shell_with_system(None), vec![result("rg", &[], None)]);
    press(&mut app, alt_enter());
    update(
        &mut app,
        Action::VersionsLoaded("rg".into(), Err("no index".into())),
    );
    let Stage::Versions(picker) = &search(&app).stage else {
        panic!("expected the version picker");
    };
    assert!(!picker.loading);
    assert_eq!(picker.error.as_deref(), Some("no index"));
}

#[test]
fn picking_a_historical_version_in_flake_mode_offers_a_new_input() {
    let app = with_results(flake_app("devshells"), vec![result("rg", &[], None)]);
    let mut app = pick_versions(app, vec![version("13.0", "0011223344556677")]);
    press(&mut app, key(KeyCode::Enter));
    match &app.popup {
        Popup::AddInput(form) => {
            assert_eq!(form.name, "nixpkgs-0011223");
            assert_eq!(form.url, "github:NixOS/nixpkgs/0011223344556677");
        }
        _ => panic!("expected the add-input form"),
    }
    assert_eq!(app.mode, Mode::Flake);
}

fn flake_result(name: &str, input: &str, version: &str) -> SearchResult {
    SearchResult {
        source_input: Some(input.to_string()),
        ..result(name, &[(input, version)], None)
    }
}

fn shell_with_flake() -> App {
    let mut app = shell_with_system(Some("sysrev"));
    app.shell.flakes.push(Input {
        name: "nixvim".into(),
        url: "github:nix-community/nixvim".into(),
        ..Default::default()
    });
    app
}

#[test]
fn shell_searches_include_the_custom_flakes() {
    let mut app = open(shell_with_flake());
    type_text(&mut app, "vim");
    std::thread::sleep(DEBOUNCE);
    update(&mut app, Action::Tick);
    match app.effects.as_slice() {
        [Effect::Search { request, .. }] => match &request.scope {
            Scope::Shell { flakes, .. } => assert_eq!(flakes, &app.shell.flakes),
            Scope::Flake { .. } => panic!("expected a shell scope"),
        },
        _ => panic!("expected one Search effect"),
    }
}

#[test]
fn enter_on_a_custom_flake_result_adds_it_from_that_flake() {
    let mut app = with_results(
        shell_with_flake(),
        vec![flake_result("nixvim.default", "nixvim", "0.1")],
    );
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    let added = &app.shell.packages[0];
    assert_eq!(
        added.reference.source,
        Source::Flake {
            name: "nixvim".into(),
            url: "github:nix-community/nixvim".into(),
        }
    );
    assert_eq!(added.reference.attr, "default");
    assert_eq!(added.reference.version.as_deref(), Some("0.1"));
    assert_eq!(added.description, "nixvim.default description");
    assert!(
        !app.effects
            .iter()
            .any(|e| matches!(e, Effect::ShellMetadata(_))),
        "nix-env knows nothing about custom flakes"
    );
}

#[test]
fn custom_flake_results_have_no_version_history() {
    let mut app = with_results(
        shell_with_flake(),
        vec![flake_result("nixvim.default", "nixvim", "0.1")],
    );
    press(&mut app, alt_enter());
    assert!(matches!(search(&app).stage, Stage::Results));
    assert!(app.effects.is_empty());
}
