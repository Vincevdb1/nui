mod common;

use common::{char, key, press, shell_app, type_text};
use nui::app::update::update;
use nui::app::{Action, App, Effect, Exit, Popup};
use nui::backend::nix::Metadata;
use nui::backend::system::SystemInfo;
use nui::features::search::Stage;
use nui::features::shell::flakes::FlakeList;
use nui::features::shell::{ShellPackage, ShellState, flake_name};
use nui::pkgref::{PackageRef, Source};
use ratatui::crossterm::event::KeyCode;

fn reference(attr: &str) -> PackageRef {
    PackageRef::new(Source::Nixpkgs("rev".into()), attr, Some("1.0".into()))
}

fn shell_package(attr: &str) -> ShellPackage {
    ShellPackage {
        reference: reference(attr),
        description: String::new(),
        is_unfree: false,
    }
}

fn state(attrs: &[&str]) -> ShellState {
    let mut state = ShellState::default();
    for attr in attrs {
        state.add(shell_package(attr));
    }
    state
}

fn attrs(state: &ShellState) -> Vec<&str> {
    state
        .packages
        .iter()
        .map(|p| p.reference.attr.as_str())
        .collect()
}

fn app_with(packages: &[&str]) -> App {
    let mut app = shell_app();
    app.shell = state(packages);
    app.shell.table.select(Some(0));
    app
}

#[test]
fn adding_skips_duplicates() {
    let mut s = ShellState::default();
    assert!(s.add(shell_package("git")));
    assert!(!s.add(shell_package("git")));
    assert!(s.add(ShellPackage {
        reference: PackageRef::new(Source::Registry, "git", None),
        ..shell_package("git")
    }));
    assert_eq!(
        s.packages.len(),
        2,
        "same attr from another source is a different package"
    );
    assert!(s.contains(&reference("git")));
}

#[test]
fn removing_one_package_keeps_the_selection_in_range() {
    let mut s = state(&["a", "b", "c"]);
    s.remove(&[2]);
    assert_eq!(attrs(&s), ["a", "b"]);
    assert_eq!(s.table.selected(), Some(1));

    s.remove(&[0]);
    assert_eq!(s.table.selected(), Some(0));
    s.remove(&[0]);
    assert_eq!(s.table.selected(), None);
    assert!(s.packages.is_empty());
}

#[test]
fn removing_several_packages_handles_any_order() {
    let mut s = state(&["a", "b", "c", "d"]);
    s.marked.extend([0, 2]);
    s.remove(&[0, 2, 2, 9]);
    assert_eq!(attrs(&s), ["b", "d"]);
    assert!(s.marked.is_empty());
    assert_eq!(s.table.selected(), Some(0));
}

#[test]
fn selection_ignores_stale_indices() {
    let mut s = state(&["a"]);
    s.table.select(Some(3));
    assert_eq!(s.selected(), None);
}

#[test]
fn metadata_fills_in_version_and_description() {
    let mut app = app_with(&["git"]);
    update(
        &mut app,
        Action::ShellMetadata(
            reference("git"),
            Metadata {
                version: "2.50".into(),
                description: "VCS".into(),
            },
        ),
    );
    let pkg = &app.shell.packages[0];
    assert_eq!(pkg.reference.version.as_deref(), Some("2.50"));
    assert_eq!(pkg.description, "VCS");
}

#[test]
fn s_starts_a_shell_with_every_package() {
    let mut app = app_with(&["git", "jq"]);
    press(&mut app, char('s'));
    match &app.exit {
        Some(Exit::Shell(refs)) => assert_eq!(refs, &[reference("git"), reference("jq")]),
        _ => panic!("expected to exit into a shell"),
    }
}

#[test]
fn d_removes_the_selected_or_marked_packages() {
    let mut app = app_with(&["a", "b", "c"]);
    press(&mut app, char('d'));
    assert_eq!(attrs(&app.shell), ["b", "c"]);

    press(&mut app, char(' '));
    press(&mut app, char('j'));
    press(&mut app, char(' '));
    press(&mut app, char('d'));
    assert!(app.shell.packages.is_empty());
}

#[test]
fn j_and_k_wrap_through_packages() {
    let mut app = app_with(&["a", "b"]);
    press(&mut app, char('k'));
    assert_eq!(app.shell.selected(), Some(1));
    press(&mut app, char('j'));
    assert_eq!(app.shell.selected(), Some(0));
}

#[test]
fn v_opens_the_version_picker_for_the_selected_package() {
    let mut app = app_with(&["git"]);
    press(&mut app, char('v'));
    match &app.popup {
        Popup::Search(s) => match &s.stage {
            Stage::Versions(picker) => {
                assert_eq!(picker.package, "git");
                assert_eq!(picker.replacing, Some(0));
            }
            _ => panic!("expected the version stage"),
        },
        _ => panic!("expected the search popup"),
    }
    assert!(matches!(app.effects.last(), Some(Effect::FetchVersions(a)) if a == "git"));
}

#[test]
fn i_shows_details_and_capital_t_offers_saving() {
    let mut app = app_with(&["git"]);
    press(&mut app, char('i'));
    match &app.popup {
        Popup::Details(result) => {
            assert_eq!(result.name, "git");
            assert_eq!(result.hash.as_deref(), Some("rev"));
        }
        _ => panic!("expected details"),
    }
    app.popup = Popup::None;
    press(&mut app, char('T'));
    assert!(matches!(app.popup, Popup::SaveTemplate(_)));
}

#[test]
fn keys_on_an_empty_shell_are_harmless() {
    let mut app = shell_app();
    for c in ['d', 'v', 'i', ' ', 'j', 'k'] {
        press(&mut app, char(c));
    }
    assert!(app.shell.packages.is_empty());
    assert!(matches!(app.popup, Popup::None));
}

#[test]
fn template_packages_use_the_system_nixpkgs_when_known() {
    let mut app = shell_app();
    update(
        &mut app,
        Action::ShellTemplateLoaded(vec!["git".into(), "jq".into()]),
    );
    assert!(matches!(app.popup, Popup::None));
    assert!(
        app.shell
            .packages
            .iter()
            .all(|p| p.reference.source == Source::Registry)
    );
    assert_eq!(
        app.effects.len(),
        2,
        "metadata is fetched for each added package"
    );

    let mut app = shell_app();
    app.system = Some(SystemInfo {
        nixpkgs_rev: Some("sysrev".into()),
        ..Default::default()
    });
    update(
        &mut app,
        Action::ShellTemplateLoaded(vec!["git".into(), "git".into()]),
    );
    assert_eq!(app.shell.packages.len(), 1);
    assert_eq!(
        app.shell.packages[0].reference.source,
        Source::System("sysrev".into())
    );
}

fn flake_package(name: &str, url: &str) -> ShellPackage {
    ShellPackage {
        reference: PackageRef::new(
            Source::Flake {
                name: name.into(),
                url: url.into(),
            },
            "default",
            None,
        ),
        ..shell_package("default")
    }
}

fn flake_list(app: &App) -> &FlakeList {
    match &app.popup {
        Popup::ShellFlakes(list) => list,
        _ => panic!("the flake list is not open"),
    }
}

fn add_flake(app: &mut App, url: &str) {
    press(app, char('a'));
    type_text(app, url);
    press(app, key(KeyCode::Enter));
}

fn flake_names(app: &App) -> Vec<&str> {
    app.shell.flakes.iter().map(|f| f.name.as_str()).collect()
}

#[test]
fn flake_names_come_from_the_reference() {
    let cases = [
        ("github:nix-community/nixvim", "nixvim"),
        ("github:nix-community/nixvim/main?dir=sub", "nixvim"),
        ("git+https://example.com/tools.git", "tools"),
        ("path:/home/me/my-tools/", "my-tools"),
        ("nixvim", "nixvim"),
        (".", "flake"),
    ];
    for (url, name) in cases {
        assert_eq!(flake_name(url), name, "url {url}");
    }
}

#[test]
fn f_opens_the_flake_list_and_esc_closes_it() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    assert!(flake_list(&app).input.is_none());
    press(&mut app, key(KeyCode::Esc));
    assert!(matches!(app.popup, Popup::None));
}

#[test]
fn typed_flakes_are_added_to_the_list() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    add_flake(&mut app, " github:nix-community/nixvim ");
    assert_eq!(flake_names(&app), ["nixvim"]);
    assert_eq!(app.shell.flakes[0].url, "github:nix-community/nixvim");
    let list = flake_list(&app);
    assert!(list.input.is_none(), "the list is shown again after adding");
    assert_eq!(list.list.selected(), Some(0));
}

#[test]
fn esc_while_typing_a_flake_returns_to_the_list() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    press(&mut app, char('a'));
    type_text(&mut app, "github:a/b");
    press(&mut app, key(KeyCode::Esc));
    assert!(flake_list(&app).input.is_none());
    assert!(app.shell.flakes.is_empty());
}

#[test]
fn empty_and_duplicate_flakes_are_rejected() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    add_flake(&mut app, "  ");
    assert!(app.shell.flakes.is_empty());
    assert!(flake_list(&app).input.is_some(), "still asking for a flake");
    press(&mut app, key(KeyCode::Esc));

    add_flake(&mut app, "github:a/tools");
    add_flake(&mut app, "github:a/tools");
    assert_eq!(flake_names(&app), ["tools"]);
    assert!(flake_list(&app).error.is_some());
}

#[test]
fn flakes_with_the_same_name_get_a_suffix() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    add_flake(&mut app, "github:a/tools");
    add_flake(&mut app, "github:b/tools");
    add_flake(&mut app, "github:c/nxv");
    assert_eq!(flake_names(&app), ["tools", "tools-2", "nxv-2"]);
}

#[test]
fn d_removes_the_selected_flake_but_keeps_its_packages() {
    let mut app = shell_app();
    press(&mut app, char('f'));
    add_flake(&mut app, "github:a/one");
    add_flake(&mut app, "github:a/two");
    app.shell.add(flake_package("two", "github:a/two"));
    assert_eq!(flake_list(&app).list.selected(), Some(1));
    press(&mut app, char('d'));
    assert_eq!(flake_names(&app), ["one"]);
    assert_eq!(flake_list(&app).list.selected(), Some(0));
    assert_eq!(app.shell.packages.len(), 1);
    press(&mut app, char('d'));
    press(&mut app, char('d'));
    assert!(app.shell.flakes.is_empty());
    assert_eq!(flake_list(&app).list.selected(), None);
}

#[test]
fn v_does_nothing_for_flake_packages() {
    let mut app = shell_app();
    app.shell.add(flake_package("two", "github:a/two"));
    app.shell.table.select(Some(0));
    press(&mut app, char('v'));
    assert!(matches!(app.popup, Popup::None));
    assert!(app.effects.is_empty());
}

#[test]
fn saved_templates_leave_out_flake_packages() {
    let mut app = app_with(&["git"]);
    app.shell.add(flake_package("two", "github:a/two"));
    press(&mut app, char('T'));
    type_text(&mut app, "mine");
    press(&mut app, key(KeyCode::Enter));
    match app.effects.as_slice() {
        [Effect::SaveTemplate { packages, .. }] => assert_eq!(packages, &["git"]),
        _ => panic!("expected one SaveTemplate effect"),
    }
}
