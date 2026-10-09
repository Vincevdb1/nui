use crate::app::{App, Exit, Popup};
use crate::features::{details, search, templates};
use crate::ui::widgets::Selectable;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub fn on_key(app: &mut App, key: KeyEvent) {
    let shell = &mut app.shell;
    match key.code {
        KeyCode::Char('a') => search::open(app),
        KeyCode::Char('s') => app.exit = Some(Exit::Shell(shell.references())),
        KeyCode::Char('T') => templates::open_save(app),
        KeyCode::Char('v') => {
            if let Some(i) = shell.selected() {
                search::open_versions(app, i);
            }
        }
        KeyCode::Char(' ') => {
            if let Some(i) = shell.selected()
                && !shell.marked.remove(&i)
            {
                shell.marked.insert(i);
            }
        }
        KeyCode::Char('d') => {
            let indices: Vec<usize> = if shell.marked.is_empty() {
                shell.selected().into_iter().collect()
            } else {
                shell.marked.iter().copied().collect()
            };
            if !indices.is_empty() {
                shell.remove(&indices);
            }
        }
        KeyCode::Char('i') => {
            if let Some(pkg) = shell.selected().map(|i| &shell.packages[i]) {
                let r = &pkg.reference;
                app.popup = Popup::Details(details::installed(
                    &r.attr,
                    &pkg.description,
                    r.version.as_deref().unwrap_or("Unknown"),
                    "shell",
                    pkg.is_unfree,
                    None,
                    r.rev().map(str::to_string),
                ));
            }
        }
        KeyCode::Char('j' | 'J') | KeyCode::Down => shell.table.step(shell.packages.len(), true),
        KeyCode::Char('k' | 'K') | KeyCode::Up => shell.table.step(shell.packages.len(), false),
        _ => {}
    }
}
