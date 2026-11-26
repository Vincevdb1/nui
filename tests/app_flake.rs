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
        [Effect::CheckUpdates(jobs)] => {
            assert_eq!(jobs, &[("git".to_string(), "nixos-unstable".to_string())]);
        }
        _ => panic!("expected CheckUpdates"),
    }
}

fn cache(app: &mut App, attr: &str, version: &str) {
    app.latest_versions.insert(
        (attr.to_string(), "nixos-unstable".to_string()),
        version.to_string(),
    );
}

#[test]
fn cached_versions_are_shown_without_a_lookup() {
    let mut app = flake_app("pinned");
    cache(&mut app, "git", "2.51");
    let generation = fetch(&mut app);
    update(
        &mut app,
        Action::PackagesLoaded(
            generation,
            Ok(vec![package("git", "2.50"), package("curl", "8.0")]),
        ),
    );
    assert_eq!(
        app.flake.updates.get("git").map(String::as_str),
        Some("2.51")
    );
    match app.effects.as_slice() {
        [Effect::CheckUpdates(jobs)] => {
            assert_eq!(jobs, &[("curl".to_string(), "nixos-unstable".to_string())]);
        }
        _ => panic!("expected CheckUpdates for the uncached package"),
    }
}

#[test]
fn no_lookup_runs_when_every_version_is_cached() {
    let mut app = flake_app("pinned");
    cache(&mut app, "git", "2.51");
    let generation = fetch(&mut app);
    update(
        &mut app,
        Action::PackagesLoaded(generation, Ok(vec![package("git", "2.50")])),
    );
    assert!(app.effects.is_empty());
    assert_eq!(
        app.flake.updates.get("git").map(String::as_str),
        Some("2.51")
    );
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
                ..
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

fn inputs_app() -> App {
    let mut app = flake_app("devshells");
    app.focus = Focus::Inputs;
    app.flake.input_table.select(Some(0));
    app
}

#[test]
fn u_updates_the_selected_input() {
    let mut app = inputs_app();
    let name = app.flake.inputs[0].name.clone();
    press(&mut app, char('u'));
    match app.effects.as_slice() {
        [Effect::UpdateInputs { inputs, before, .. }] => {
            assert_eq!(inputs, &[name]);
            assert_eq!(before, &app.flake.inputs);
        }
        _ => panic!("expected UpdateInputs"),
    }
    assert!(app.flake.updating);
}

#[test]
fn shift_u_updates_all_inputs() {
    let mut app = inputs_app();
    press(&mut app, char('U'));
    assert!(
        matches!(app.effects.as_slice(), [Effect::UpdateInputs { inputs, .. }] if inputs.is_empty())
    );
}

#[test]
fn a_second_update_waits_for_the_first() {
    let mut app = inputs_app();
    press(&mut app, char('u'));
    app.effects.clear();
    press(&mut app, char('U'));
    assert!(app.effects.is_empty());
}

#[test]
fn u_without_a_selected_input_does_nothing() {
    let mut app = inputs_app();
    app.flake.input_table.select(None);
    press(&mut app, char('u'));
    assert!(app.effects.is_empty());
    assert!(!app.flake.updating);
}

#[test]
fn a_finished_update_reloads_the_flake() {
    let mut app = inputs_app();
    app.flake.updating = true;
    update(&mut app, Action::InputsUpdated(Ok(vec![])));
    assert!(!app.flake.updating);
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::LoadFlake(_)))
    );
}

#[test]
fn a_failed_update_does_not_reload() {
    let mut app = inputs_app();
    app.flake.updating = true;
    update(&mut app, Action::InputsUpdated(Err("offline".into())));
    assert!(!app.flake.updating);
    assert!(app.effects.is_empty());
}

fn shell(path: &str) -> nui::flake::Output {
    nui::flake::Output {
        kind: nui::flake::OutputKind::DevShell,
        path: path.into(),
        name: None,
        content: None,
        edit_error: None,
    }
}

#[test]
fn a_package_edit_checks_only_its_shell() {
    let edit = FlakeEdit::AddPackage {
        target: x86("default"),
        package: "jq".into(),
    };
    let outputs = [shell("x86_64-linux.default"), shell("x86_64-linux.other")];
    assert_eq!(flake::check_targets(&edit, &outputs), [x86("default")]);
}

#[test]
fn an_input_edit_checks_every_shell() {
    let edit = FlakeEdit::RemoveInput("nixpkgs-stable".into());
    let mut nixos = shell("desktop");
    nixos.kind = nui::flake::OutputKind::Nixos;
    let outputs = [
        shell("x86_64-linux.default"),
        nixos,
        shell("x86_64-linux.other"),
    ];
    assert_eq!(
        flake::check_targets(&edit, &outputs),
        [x86("default"), x86("other")]
    );
}

#[test]
fn removing_a_package_sends_its_shell_as_the_check() {
    let mut app = loaded("devshells", 0, vec![package("git", "2.50")]);
    let expected = app.flake.outputs[0].shell_target().unwrap();
    press(&mut app, char('d'));
    match app.effects.as_slice() {
        [Effect::Edit { check, .. }] => assert_eq!(check, &[expected]),
        _ => panic!("expected an Edit effect"),
    }
}

#[test]
fn an_applied_edit_can_be_undone() {
    let mut app = flake_app("devshells");
    update(&mut app, Action::EditApplied(Default::default()));
    assert!(app.flake.last_edit.is_some());
}
