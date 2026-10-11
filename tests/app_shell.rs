mod common;

use common::{char, press, shell_app};
use nui::app::update::update;
use nui::app::{Action, App, Effect, Exit, Popup};
use nui::backend::nix::Metadata;
use nui::backend::system::SystemInfo;
use nui::features::search::Stage;
use nui::features::shell::{ShellPackage, ShellState};
use nui::pkgref::{PackageRef, Source};

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
