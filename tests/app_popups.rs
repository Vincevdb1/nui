mod common;

use common::{char, flake_app, key, press, shell_app, type_text, x86};
use nui::app::effect::FlakeEdit;
use nui::app::update::update;
use nui::app::{Action, App, Effect, Popup};
use nui::backend::github::rank_branches;
use nui::backend::templates::Template;
use nui::features::add_input::{AddInput, Field};
use nui::features::{add_input, details, templates};
use ratatui::crossterm::event::KeyCode;

fn add_input_form(app: &App) -> &AddInput {
    match &app.popup {
        Popup::AddInput(form) => form,
        _ => panic!("the add-input form is not open"),
    }
}

fn open_add_input(fixture: &str) -> App {
    let mut app = flake_app(fixture);
    app.popup = add_input::open(&mut app, None);
    app.effects.clear();
    app
}

fn branches() -> Vec<String> {
    ["nixos-25.11", "nixos-24.05", "nixpkgs-unstable"]
        .map(String::from)
        .to_vec()
}

#[test]
fn add_input_filters_suggestions_by_name_and_existing_urls() {
    let mut app = open_add_input("nixos"); // already tracks nixos-25.11
    update(
        &mut app,
        Action::BranchesLoaded(Ok(rank_branches(branches()))),
    );
    let form = add_input_form(&app);
    assert!(!form.loading);
    let names: Vec<_> = form.suggestions.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["nixpkgs-unstable", "nixos-24.05"]);

    type_text(&mut app, "24");
    let names: Vec<_> = add_input_form(&app)
        .suggestions
        .iter()
        .map(|b| b.name.as_str())
        .collect();
    assert_eq!(names, ["nixos-24.05"]);
    assert_eq!(add_input_form(&app).list.selected(), Some(0));
}

#[test]
fn add_input_shows_branch_errors() {
    let mut app = open_add_input("nixos");
    update(&mut app, Action::BranchesLoaded(Err("offline".into())));
    assert_eq!(add_input_form(&app).error.as_deref(), Some("offline"));
    assert!(!add_input_form(&app).loading);
}

#[test]
fn add_input_fields_cycle_with_tab() {
    let mut app = open_add_input("nixos");
    assert!(add_input_form(&app).field == Field::Name);
    press(&mut app, key(KeyCode::Tab));
    assert!(add_input_form(&app).field == Field::Url);
    press(&mut app, key(KeyCode::Tab));
    assert!(add_input_form(&app).field == Field::Suggestions);
    press(&mut app, key(KeyCode::BackTab));
    assert!(add_input_form(&app).field == Field::Url);
}

#[test]
fn add_input_submits_typed_name_and_url() {
    let mut app = open_add_input("nixos");
    type_text(&mut app, "tools");
    press(&mut app, key(KeyCode::Tab));
    type_text(&mut app, "github:me/tools");
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::Edit { edit: FlakeEdit::AddInput { name, url, package: None }, .. }]
            if name == "tools" && url == "github:me/tools"
    ));
}

#[test]
fn add_input_needs_both_name_and_url() {
    let mut app = open_add_input("nixos");
    type_text(&mut app, "tools");
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::AddInput(_)));
    assert!(app.effects.is_empty());
}

#[test]
fn add_input_submits_the_highlighted_suggestion() {
    let mut app = open_add_input("nixos");
    update(
        &mut app,
        Action::BranchesLoaded(Ok(rank_branches(branches()))),
    );
    press(&mut app, key(KeyCode::BackTab)); // Name -> Suggestions
    press(&mut app, char('j'));
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::Edit { edit: FlakeEdit::AddInput { name, url, .. }, .. }]
            if name == "nixos-24.05" && url == "github:nixos/nixpkgs/nixos-24.05"
    ));
}

#[test]
fn a_prefilled_input_also_adds_its_package() {
    let mut app = flake_app("devshells");
    app.popup = add_input::open(
        &mut app,
        Some((
            "nixpkgs-0011223".into(),
            "github:NixOS/nixpkgs/0011223".into(),
            "rg".into(),
        )),
    );
    press(&mut app, key(KeyCode::Enter));
    let package = app.effects.iter().find_map(|e| match e {
        Effect::Edit {
            edit: FlakeEdit::AddInput { package, .. },
            ..
        } => package.clone(),
        _ => None,
    });
    assert_eq!(package, Some((x86("default"), "rg".to_string())));
}

#[test]
fn esc_closes_add_input() {
    let mut app = open_add_input("nixos");
    press(&mut app, key(KeyCode::Esc));
    assert!(matches!(app.popup, Popup::None));
}

fn with_templates(mut app: App) -> App {
    app.templates = vec![
        Template {
            name: "python.nix".into(),
            description: "Python".into(),
        },
        Template {
            name: "rust.nix".into(),
            description: "Rust".into(),
        },
    ];
    app
}

#[test]
fn choosing_a_template_in_shell_mode_applies_it() {
    let mut app = with_templates(shell_app());
    templates::open(&mut app);
    press(&mut app, char('j'));
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::ApplyTemplate { name, shell: true }] if name == "rust.nix"
    ));
}

#[test]
fn choosing_with_no_templates_just_closes() {
    let mut app = shell_app();
    app.templates.clear();
    templates::open(&mut app);
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    assert!(app.effects.is_empty());
}

#[test]
fn overwriting_flake_nix_needs_confirmation() {
    let mut app = shell_app();
    app.popup = Popup::ConfirmTemplate("rust.nix".into());
    press(&mut app, char('x'));
    assert!(matches!(app.popup, Popup::ConfirmTemplate(_)));
    press(&mut app, char('n'));
    assert!(matches!(app.popup, Popup::None));
    assert!(app.effects.is_empty());

    app.popup = Popup::ConfirmTemplate("rust.nix".into());
    press(&mut app, char('y'));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::ApplyTemplate { name, .. }] if name == "rust.nix"
    ));
}

#[test]
fn saving_a_template_collects_the_shell_packages() {
    let mut app = shell_app();
    app.shell.add(nui::features::shell::ShellPackage {
        reference: nui::pkgref::PackageRef::new(nui::pkgref::Source::Registry, "git", None),
        description: String::new(),
        is_unfree: false,
    });
    templates::open_save(&mut app);
    press(&mut app, key(KeyCode::Enter));
    assert!(app.effects.is_empty(), "a filename is required");

    type_text(&mut app, "dev");
    press(&mut app, key(KeyCode::Tab));
    type_text(&mut app, "My devx");
    press(&mut app, key(KeyCode::Backspace));
    press(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.popup, Popup::None));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::SaveTemplate { name, description, packages }]
            if name == "dev" && description == "My dev" && packages == &["git".to_string()]
    ));
}

#[test]
fn details_close_on_q_esc_or_i() {
    for close in [char('q'), char('i'), key(KeyCode::Esc)] {
        let mut app = shell_app();
        app.popup = Popup::Details(details::installed(
            "git", "", "1", "shell", false, None, None,
        ));
        press(&mut app, char('x'));
        assert!(matches!(app.popup, Popup::Details(_)));
        press(&mut app, close);
        assert!(matches!(app.popup, Popup::None));
    }
}

#[test]
fn installed_details_have_a_single_version() {
    let result = details::installed(
        "hello",
        "greets",
        "2.12",
        "current",
        true,
        Some("nixpkgs".into()),
        Some("abc".into()),
    );
    assert_eq!(result.versions.len(), 1);
    assert_eq!(result.versions[0].channel, "current");
    assert!(result.is_unfree);
    assert_eq!(result.source_input.as_deref(), Some("nixpkgs"));
    assert_eq!(result.hash.as_deref(), Some("abc"));
}
