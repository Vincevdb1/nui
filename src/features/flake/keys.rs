use super::{fetch_packages, reload};
use crate::app::effect::FlakeEdit;
use crate::app::{App, Effect, Focus, Popup};
use crate::features::{add_input, details, search};
use crate::ui::widgets::{Selectable, wrap};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub fn on_key(app: &mut App, key: KeyEvent) {
    let focus = app.focus;
    match key.code {
        KeyCode::Tab => app.focus = focus.cycle(true),
        KeyCode::BackTab => app.focus = focus.cycle(false),
        KeyCode::Char(c) if Focus::from_digit(c).is_some() => {
            app.focus = Focus::from_digit(c).unwrap_or(focus)
        }
        KeyCode::Char('a') if focus == Focus::Context => search::open(app),
        KeyCode::Char('a') if focus == Focus::Inputs => app.popup = add_input::open(app, None),
        KeyCode::Char('d') if focus == Focus::Context => remove_packages(app),
        KeyCode::Char('d') if focus == Focus::Inputs => remove_input(app),
        KeyCode::Char(' ') if focus == Focus::Context => toggle_mark(app),
        KeyCode::Char('p') if focus == Focus::Context => toggle_pin(app),
        KeyCode::Char('i') if focus == Focus::Context => show_details(app),
        KeyCode::Char(c @ ('j' | 'k')) if focus == Focus::Context => {
            let len = app.flake.packages.len();
            app.flake.package_table.step(len, c == 'j');
        }
        KeyCode::Char(c @ ('j' | 'k')) if focus == Focus::Inputs => {
            let len = app.flake.inputs.len();
            app.flake.input_table.step(len, c == 'j');
        }
        KeyCode::Char('j' | 'J') | KeyCode::Down => move_selection(app, true),
        KeyCode::Char('k' | 'K') | KeyCode::Up => move_selection(app, false),
        _ => {}
    }
}

/// Moves the focused panel's own selection; Context switches between flake files.
fn move_selection(app: &mut App, forward: bool) {
    let flake = &mut app.flake;
    match app.focus {
        Focus::Title => {}
        Focus::Context => {
            if let Some(i) = wrap(Some(flake.file), flake.files.len(), forward) {
                flake.file = i;
                reload(app);
            }
        }
        Focus::Inputs => flake.input_table.step(flake.inputs.len(), forward),
        Focus::Outputs => {
            if let Some(i) = wrap(Some(flake.output), flake.outputs.len(), forward) {
                flake.output = i;
                fetch_packages(app);
            }
        }
        Focus::Log => app.log.scroll(forward),
    }
}

fn remove_packages(app: &mut App) {
    let flake = &mut app.flake;
    let packages: Vec<String> = if flake.marked.is_empty() {
        flake
            .selected_package()
            .map(|p| p.qualified_name())
            .into_iter()
            .collect()
    } else {
        flake
            .packages
            .iter()
            .filter(|p| flake.marked.contains(&p.name))
            .map(|p| p.qualified_name())
            .collect()
    };
    if packages.is_empty() {
        return;
    }
    if let Some((file, target)) = flake.edit_target() {
        flake.marked.clear();
        app.effects.push(Effect::Edit {
            flake: file,
            edit: FlakeEdit::RemovePackages { target, packages },
        });
    }
}

fn remove_input(app: &mut App) {
    let flake = &app.flake;
    if let (Some(file), Some(input)) = (flake.selected_file(), flake.selected_input()) {
        app.effects.push(Effect::Edit {
            flake: file.clone(),
            edit: FlakeEdit::RemoveInput(input.name.clone()),
        });
    }
}

fn toggle_mark(app: &mut App) {
    let flake = &mut app.flake;
    if let Some(name) = flake.selected_package().map(|p| p.name.clone())
        && !flake.marked.remove(&name)
    {
        flake.marked.insert(name);
    }
}

fn toggle_pin(app: &mut App) {
    let flake = &app.flake;
    let Some(package) = flake.selected_package().map(|p| p.name.clone()) else {
        return;
    };
    let Some((file, target)) = flake.edit_target() else {
        return;
    };
    let edit = if package.starts_with("inputs.") {
        FlakeEdit::Unpin { target, package }
    } else {
        FlakeEdit::Pin {
            target,
            package,
            inputs: flake.inputs.clone(),
        }
    };
    app.effects.push(Effect::Edit { flake: file, edit });
}

fn show_details(app: &mut App) {
    if let Some(p) = app.flake.selected_package() {
        app.popup = Popup::Details(details::installed(
            &p.name,
            &p.description,
            &p.version,
            "current",
            p.is_unfree,
            p.source.clone(),
            None,
        ));
    }
}
