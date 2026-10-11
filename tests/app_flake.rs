mod common;

use common::{char, flake_app, key, press, x86};
use nui::app::effect::FlakeEdit;
use nui::app::update::update;
use nui::app::{Action, App, Effect, Focus, Generation, Popup};
use nui::features::flake;
use nui::flake::Package;
use ratatui::crossterm::event::KeyCode;

fn package(name: &str, version: &str) -> Package {
    Package {
        name: name.to_string(),
        description: format!("{name} description"),
        version: version.to_string(),
        is_unfree: false,
        source: None,
        attr: None,
    }
}

/// Starts a package fetch for the selected output and returns its generation.
fn fetch(app: &mut App) -> Generation {
    app.effects.clear();
    flake::fetch_packages(app);
    match app.effects.pop() {
        Some(Effect::FetchPackages { generation, .. }) => generation,
        _ => panic!("expected a FetchPackages effect"),
    }
}

/// A flake app with the selected output's packages loaded and the first one selected.
fn loaded(fixture: &str, output: usize, packages: Vec<Package>) -> App {
    let mut app = flake_app(fixture);
    app.flake.output = output;
    let generation = fetch(&mut app);
    update(&mut app, Action::PackagesLoaded(generation, Ok(packages)));
    app.flake.package_table.select(Some(0));
    app.focus = Focus::Context;
    app.effects.clear();
    app
}

fn names(app: &App) -> Vec<&str> {
    app.flake.packages.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn fetching_targets_the_selected_output() {
    let mut app = flake_app("devshells");
    app.flake.output = 1;
    flake::fetch_packages(&mut app);
    match app.effects.as_slice() {
        [
            Effect::FetchPackages {
                flake_ref, output, ..
            },
        ] => {
            assert!(flake_ref.ends_with("test_flakes/devshells"), "{flake_ref}");
            assert_eq!(output.path, "x86_64-linux.another");
        }
        _ => panic!("expected FetchPackages"),
    }
    assert!(app.flake.loading);
}

#[test]
fn loaded_packages_recover_how_they_were_written() {
    let app = loaded(
        "pinned",
        0,
        vec![package("hello", "2.12"), package("git", "2.50")],
    );
    assert_eq!(names(&app), ["git", "hello"], "sorted by name");
    let git = &app.flake.packages[0];
    assert_eq!(git.attr.as_deref(), Some("pkgs.git"));
    assert_eq!(git.source.as_deref(), Some("pkgs"));
    let hello = &app.flake.packages[1];
    assert!(hello.is_pinned());
    assert_eq!(hello.source.as_deref(), Some("nixpkgs-hello"));
    assert!(!app.flake.loading);
}

#[test]
fn evaluated_names_are_matched_to_written_attributes_by_prefix() {
    // `python312` evaluates to a package named `python3`.
    let app = loaded("devshells", 0, vec![package("python3", "3.12.9")]);
    assert_eq!(app.flake.packages[0].attr.as_deref(), Some("python312"));
}

#[test]
fn packages_without_version_or_source_are_hidden() {
    let app = loaded(
        "devshells",
        0,
        vec![package("isort", "6.0"), package("setup-hook", "")],
    );
    assert_eq!(names(&app), ["isort"]);
}

#[test]
fn loaded_packages_trigger_an_update_check() {
    let mut app = flake_app("pinned");
    let generation = fetch(&mut app);
    update(
        &mut app,
        Action::PackagesLoaded(generation, Ok(vec![package("git", "2.50")])),
    );
    match app.effects.as_slice() {
        [Effect::CheckUpdates { packages, inputs }] => {
            assert_eq!(packages, &[("git".to_string(), Some("pkgs".to_string()))]);
            assert_eq!(inputs.len(), 2);
        }
        _ => panic!("expected CheckUpdates"),
    }
}

#[test]
fn results_of_a_superseded_fetch_are_dropped() {
    let mut app = flake_app("devshells");
    let stale = fetch(&mut app);
    let current = fetch(&mut app);
    update(
        &mut app,
        Action::PackagesLoaded(stale, Ok(vec![package("isort", "1")])),
    );
    assert!(app.flake.packages.is_empty());
    assert!(app.flake.loading);

    update(
        &mut app,
        Action::PackagesLoaded(current, Err("eval failed".into())),
    );
    assert!(!app.flake.loading);
    assert_eq!(app.flake.error.as_deref(), Some("eval failed"));
}

#[test]
fn flake_loaded_keeps_a_valid_output_selection() {
    let mut app = flake_app("devshells");
    app.flake.output = 5;
    let (inputs, outputs) = (app.flake.inputs.clone(), app.flake.outputs.clone());
    update(&mut app, Action::FlakeLoaded(inputs, outputs));
    assert_eq!(app.flake.output, 0);
    assert!(matches!(
        app.effects.last(),
        Some(Effect::FetchPackages { .. })
    ));
}

#[test]
fn tab_and_digits_move_focus() {
    let mut app = flake_app("devshells");
    app.focus = Focus::Title;
    press(&mut app, key(KeyCode::Tab));
    assert_eq!(app.focus, Focus::Context);
    press(&mut app, key(KeyCode::BackTab));
    assert_eq!(app.focus, Focus::Title);
    press(&mut app, char('5'));
    assert_eq!(app.focus, Focus::Log);
}

#[test]
fn j_and_k_move_within_the_focused_panel() {
    let mut app = loaded(
        "devshells",
        0,
        vec![package("isort", "1"), package("python3", "3")],
    );
    press(&mut app, char('j'));
    assert_eq!(app.flake.package_table.selected(), Some(1));
    press(&mut app, char('j'));
    assert_eq!(app.flake.package_table.selected(), Some(0));

    app.focus = Focus::Outputs;
    press(&mut app, char('j'));
    assert_eq!(app.flake.output, 1);
    assert!(
        matches!(app.effects.last(), Some(Effect::FetchPackages { output, .. }) if output.path == "x86_64-linux.another")
    );
    press(&mut app, key(KeyCode::Up));
    assert_eq!(app.flake.output, 0);
}

#[test]
fn d_removes_the_selected_package() {
    let mut app = loaded(
        "pinned",
        0,
        vec![package("git", "1"), package("hello", "2")],
    );
    press(&mut app, char('d'));
    match app.effects.as_slice() {
        [
            Effect::Edit {
                flake,
                edit: FlakeEdit::RemovePackages { target, packages },
            },
        ] => {
            assert_eq!(flake, &common::flake_path("pinned"));
            assert_eq!(target, &x86("default"));
            assert_eq!(packages, &["pkgs.git".to_string()]);
        }
        _ => panic!("expected a RemovePackages edit"),
    }
}

#[test]
fn space_marks_packages_for_removal() {
    let mut app = loaded(
        "pinned",
        0,
        vec![package("git", "1"), package("hello", "2")],
    );
    press(&mut app, char(' '));
    press(&mut app, char('j'));
    press(&mut app, char(' '));
    press(&mut app, char('d'));
    match app.effects.as_slice() {
        [
            Effect::Edit {
                edit: FlakeEdit::RemovePackages { packages, .. },
                ..
            },
        ] => {
            assert_eq!(
                packages,
                &[
                    "pkgs.git".to_string(),
                    "inputs.nixpkgs-hello.legacyPackages.${system}.hello".to_string()
                ]
            );
        }
        _ => panic!("expected a RemovePackages edit"),
    }
    assert!(
        app.flake.marked.is_empty(),
        "marks are cleared after removal"
    );
}

#[test]
fn space_twice_unmarks() {
    let mut app = loaded("pinned", 0, vec![package("git", "1")]);
    press(&mut app, char(' '));
    assert!(app.flake.marked.contains("git"));
    press(&mut app, char(' '));
    assert!(app.flake.marked.is_empty());
}

#[test]
fn p_pins_or_unpins_depending_on_the_package() {
    let mut app = loaded(
        "pinned",
        0,
        vec![package("git", "1"), package("hello", "2")],
    );
    press(&mut app, char('p'));
    assert!(matches!(
        app.effects.pop(),
        Some(Effect::Edit { edit: FlakeEdit::Pin { package, inputs, .. }, .. })
            if package == "pkgs.git" && inputs.len() == 2
    ));

    press(&mut app, char('j'));
    press(&mut app, char('p'));
    assert!(matches!(
        app.effects.pop(),
        Some(Effect::Edit { edit: FlakeEdit::Unpin { package, .. }, .. })
            if package.starts_with("inputs.nixpkgs-hello.")
    ));
}

#[test]
fn only_devshells_can_be_edited() {
    // mixed's outputs read from source: packages, the NixOS server, then two devShells.
    let mut app = loaded("mixed", 1, vec![package("htop", "3.4")]);
    assert_eq!(app.flake.selected_output().unwrap().path, "server");
    press(&mut app, char('d'));
    press(&mut app, char('p'));
    assert!(app.effects.is_empty());
}

#[test]
fn d_on_inputs_removes_the_selected_input() {
    let mut app = flake_app("pinned");
    app.focus = Focus::Inputs;
    press(&mut app, char('j'));
    press(&mut app, char('j'));
    press(&mut app, char('d'));
    assert!(matches!(
        app.effects.as_slice(),
        [Effect::Edit { edit: FlakeEdit::RemoveInput(name), .. }] if name == "nixpkgs-hello"
    ));
}

#[test]
fn a_opens_search_or_add_input_by_focus() {
    let mut app = flake_app("devshells");
    app.focus = Focus::Context;
    press(&mut app, char('a'));
    assert!(matches!(app.popup, Popup::Search(_)));

    app.popup = Popup::None;
    app.focus = Focus::Inputs;
    press(&mut app, char('a'));
    assert!(matches!(app.popup, Popup::AddInput(_)));
    assert!(matches!(app.effects.last(), Some(Effect::FetchBranches)));
}

#[test]
fn i_shows_details_of_the_selected_package() {
    let mut app = loaded("pinned", 0, vec![package("hello", "2.12")]);
    press(&mut app, char('i'));
    match &app.popup {
        Popup::Details(result) => {
            assert_eq!(result.name, "hello");
            assert_eq!(result.versions[0].version, "2.12");
            assert_eq!(result.source_input.as_deref(), Some("nixpkgs-hello"));
        }
        _ => panic!("expected the details popup"),
    }
    press(&mut app, char('q'));
    assert!(matches!(app.popup, Popup::None));
    assert!(app.exit.is_none());
}
