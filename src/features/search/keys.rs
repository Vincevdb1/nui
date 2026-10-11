use super::versions::{VersionItem, VersionPicker};
use super::{Search, Stage, close};
use crate::app::effect::{FlakeEdit, input_package};
use crate::app::{App, Effect, Mode, Popup};
use crate::backend::search::SearchResult;
use crate::features::add_input;
use crate::features::shell::ShellPackage;
use crate::pkgref::{PackageRef, Source};
use crate::ui::widgets::Selectable;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn on_key(app: &mut App, mut s: Search, key: KeyEvent) -> Popup {
    match std::mem::replace(&mut s.stage, Stage::Results) {
        Stage::Results => results_key(app, s, key),
        Stage::Details => {
            if !matches!(key.code, KeyCode::Esc | KeyCode::Tab | KeyCode::Char('q')) {
                s.stage = Stage::Details;
            }
            Popup::Search(s)
        }
        Stage::Versions(picker) => versions_key(app, s, picker, key),
    }
}

fn results_key(app: &mut App, mut s: Search, key: KeyEvent) -> Popup {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return close(app),
        KeyCode::Tab if !s.results.is_empty() => s.stage = Stage::Details,
        KeyCode::Char('j') if ctrl => s.list.step(s.results.len(), true),
        KeyCode::Char('k') if ctrl => s.list.step(s.results.len(), false),
        KeyCode::Down => s.list.step(s.results.len(), true),
        KeyCode::Up => s.list.step(s.results.len(), false),
        KeyCode::Backspace => s.edit(|q| {
            q.pop();
        }),
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
            // nxv only has history for nixpkgs, not for the shell's custom flakes.
            let custom_flake =
                |r: &SearchResult| app.mode == Mode::Shell && r.source_input.is_some();
            if let Some(result) = s.selected().filter(|r| !custom_flake(r)) {
                let picker = VersionPicker::new(
                    result.name.clone(),
                    result.description.clone(),
                    result.is_unfree,
                    None,
                );
                app.effects.push(Effect::FetchVersions(result.name.clone()));
                s.stage = Stage::Versions(picker);
            }
        }
        KeyCode::Enter => {
            if s.selected().is_some() {
                add_selected(app, &s);
                return close(app);
            }
        }
        KeyCode::Char(c) => s.edit(|q| q.push(c)),
        _ => {}
    }
    Popup::Search(s)
}

fn add_selected(app: &mut App, s: &Search) {
    let Some(result) = s.selected() else {
        return;
    };
    match app.mode {
        Mode::Shell if result.source_input.is_some() => add_from_flake(app, result),
        Mode::Shell => {
            let version = result.shell_version(app.system_version());
            let source = match (app.system_rev(), &result.hash) {
                (Some(rev), _) => Source::System(rev.to_string()),
                (None, Some(hash)) => Source::Nixpkgs(hash.clone()),
                (None, None) => Source::Registry,
            };
            let reference = PackageRef::new(source, result.name.clone(), Some(version));
            let added = app.shell.add(ShellPackage {
                reference: reference.clone(),
                description: result.description.clone(),
                is_unfree: result.is_unfree,
            });
            if added {
                app.effects.push(Effect::ShellMetadata(reference));
            }
        }
        Mode::Flake => add_to_flake(app, result.name.clone()),
    }
}

/// Adds a hit from one of the shell's custom flakes, which is named `<flake>.<attr>`.
fn add_from_flake(app: &mut App, result: &SearchResult) {
    let Some(flake) = app
        .shell
        .flakes
        .iter()
        .find(|f| Some(&f.name) == result.source_input.as_ref())
    else {
        return;
    };
    let attr = result
        .name
        .strip_prefix(&format!("{}.", flake.name))
        .unwrap_or(&result.name);
    let source = Source::Flake {
        name: flake.name.clone(),
        url: flake.url.clone(),
    };
    let version = result.versions.first().map(|v| v.version.clone());
    app.shell.add(ShellPackage {
        reference: PackageRef::new(source, attr, version),
        description: result.description.clone(),
        is_unfree: result.is_unfree,
    });
}

fn add_to_flake(app: &mut App, package: String) {
    if let Some((flake, target)) = app.flake.edit_target() {
        app.effects.push(Effect::Edit {
            flake,
            edit: FlakeEdit::AddPackage { target, package },
        });
    }
}

fn versions_key(app: &mut App, mut s: Search, mut picker: VersionPicker, key: KeyEvent) -> Popup {
    match key.code {
        KeyCode::Esc if picker.replacing.is_some() => return close(app),
        KeyCode::Esc => return Popup::Search(s),
        KeyCode::Down | KeyCode::Char('j') => picker.step(true),
        KeyCode::Up | KeyCode::Char('k') => picker.step(false),
        KeyCode::Enter => match picker.selected() {
            Some(VersionItem::Input { input, .. }) => {
                let package = input_package(input, &picker.package);
                add_to_flake(app, package);
                return close(app);
            }
            Some(VersionItem::Version(v)) if app.mode == Mode::Shell => {
                let source = if v.is_system {
                    Source::System(v.hash.clone())
                } else {
                    Source::Nixpkgs(v.hash.clone())
                };
                let package = ShellPackage {
                    reference: PackageRef::new(
                        source,
                        picker.package.clone(),
                        Some(v.version.clone()),
                    ),
                    description: picker.description.clone(),
                    is_unfree: picker.is_unfree,
                };
                match picker.replacing {
                    Some(i) if i < app.shell.packages.len() => app.shell.packages[i] = package,
                    Some(_) => {}
                    None => {
                        app.shell.add(package);
                    }
                }
                return close(app);
            }
            Some(VersionItem::Version(v)) => {
                let short: String = v.hash.chars().take(7).collect();
                let name = format!("nixpkgs-{}", short);
                let url = format!("github:NixOS/nixpkgs/{}", v.hash);
                let package = picker.package.clone();
                close(app);
                return add_input::open(app, Some((name, url, package)));
            }
            None => {}
        },
        _ => {}
    }
    s.stage = Stage::Versions(picker);
    Popup::Search(s)
}
