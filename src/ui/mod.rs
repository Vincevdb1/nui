pub mod widgets;

use crate::app::{App, Focus, Mode, Popup};
use crate::features::{add_input, details, flake, help, search, shell, templates};
use ratatui::{prelude::*, widgets::Paragraph};

pub fn render(app: &mut App, frame: &mut Frame) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

    match app.mode {
        Mode::Shell => shell::view::render(app, frame, body),
        Mode::Flake => flake::view::render(app, frame, body),
    }
    frame.render_widget(Paragraph::new(footer_text(app)), footer);

    let App {
        popup,
        templates: template_list,
        flake,
        system,
        mode,
        ..
    } = app;
    match popup {
        Popup::None => {}
        Popup::Help => help::render(frame),
        Popup::Templates(picker) => templates::render_picker(frame, picker, template_list),
        Popup::ConfirmTemplate(name) => templates::render_confirm(frame, name),
        Popup::SaveTemplate(form) => templates::render_save(frame, form),
        Popup::AddInput(form) => add_input::render(frame, form),
        Popup::Details(result) => details::render(frame, result),
        Popup::Search(s) => {
            let system = system.as_ref();
            let ctx = search::view::Context {
                shell_mode: *mode == Mode::Shell,
                installed: &flake.packages,
                system_version: system.and_then(|i| i.nixpkgs_version.as_deref()),
                system_rev: system.and_then(|i| i.nixpkgs_rev.as_deref()),
            };
            search::view::render(frame, s, &ctx);
        }
    }
}

fn footer_text(app: &App) -> &'static str {
    if app.mode == Mode::Shell {
        return "a: Add | d: Remove | v: Version | i: Info | s: Start Shell | m: Mode | t: Templates | T: Save Template | j/k: Select | Space: Multi-select | ?: Help | q: Quit";
    }
    match app.focus {
        Focus::Title => {
            "Tab: Switch focus | m: Switch Mode | t: Templates | 1-5: Select tab | ?: Help | q: Quit"
        }
        Focus::Context => {
            "a: Add | d: Remove | i: Info | p: Pin | m: Mode | t: Templates | Shift-j/k: Select | j/k: Navigate pkgs | Space: Multi-select | ?: Help | q: Quit"
        }
        Focus::Inputs => {
            "a: Add Input | d: Remove Input | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit"
        }
        Focus::Outputs => {
            "j/k: Select Output | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit"
        }
        Focus::Log => {
            "j/k: Scroll Logs | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit"
        }
    }
}
