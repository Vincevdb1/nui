pub mod action;
mod app;
pub mod components;
mod context;
mod nix;
pub mod state;
mod tui;
mod ui;

pub use components::command_log::{command_log, log_action, log_output};

use crate::action::Action;
use crate::app::App;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};

fn main() -> Result<()> {
    color_eyre::install()?;

    let args: Vec<String> = std::env::args().collect();
    let (mode, shell_packages) = if args.len() > 1 && args[1] == "shell" {
        (crate::state::Mode::Shell, args[2..].to_vec())
    } else {
        (crate::state::Mode::Flake, Vec::new())
    };

    let mut terminal = tui::init()?;
    let mut app = App::new(mode, shell_packages);

    let result = run(&mut terminal, &mut app);

    tui::restore()?;

    if let Ok(Some(packages)) = result {
        if !packages.is_empty() {
            let mut args = vec!["shell".to_string()];
            for pkg in &packages {
                if pkg.contains('#') {
                    args.push(pkg.clone());
                } else {
                    args.push(format!("github:NixOS/nixpkgs/nixpkgs-unstable#{}", pkg));
                }
            }
            let env_name = if packages.is_empty() {
                "nui-shell-env".to_string()
            } else {
                format!("nui-shell-{}-env", packages.join("."))
            };

            std::process::Command::new("nix")
                .args(args)
                .env("name", env_name)
                .spawn()?
                .wait()?;
        }
    }

    Ok(())
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<Option<Vec<String>>> {
    let mut shell_packages = None;
    while !app.should_quit {
        terminal.draw(|frame| ui::render(app, frame))?;

        if event::poll(std::time::Duration::from_millis(16))?
            && let Some(action) = map_event(app, event::read()?)
        {
            if let Action::StartShell(ref pkgs) = action {
                shell_packages = Some(pkgs.clone());
            }
            app.update(action);
        }
        app.update(Action::Tick);
    }
    Ok(shell_packages)
}

fn map_event(app: &App, event: Event) -> Option<Action> {
    if let Event::Key(key) = event {
        if app.ui.is_adding_package {
            if app.ui.is_showing_package_details {
                return match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Tab => {
                        Some(Action::TogglePackageDetails)
                    }
                    _ => None,
                };
            }

            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => Some(Action::ClosePopup),
                KeyCode::Tab => {
                    if !app.domain.package_search_results.is_empty() {
                        Some(Action::TogglePackageDetails)
                    } else {
                        None
                    }
                }
                KeyCode::Char('j') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    Some(Action::MoveSearchSelectionDown)
                }
                KeyCode::Char('k') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    Some(Action::MoveSearchSelectionUp)
                }
                KeyCode::Down => Some(Action::MoveSearchSelectionDown),
                KeyCode::Up => Some(Action::MoveSearchSelectionUp),
                KeyCode::Backspace => Some(Action::PackageSearchBackspace),
                KeyCode::Enter => Some(Action::PackageSearchSubmit),
                KeyCode::Char(c) => Some(Action::PackageSearchChar(c)),
                _ => None,
            };
        }

        if app.ui.is_adding_input {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') if app.ui.input_cursor == 0 => {
                    Some(Action::ClosePopup)
                }
                KeyCode::Esc => Some(Action::ClosePopup),
                KeyCode::Tab => Some(Action::NextInputField),
                KeyCode::BackTab => Some(Action::PreviousInputField),
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.ui.input_cursor == 0 {
                        Some(Action::MoveSuggestionDown)
                    } else {
                        Some(Action::NextInputField)
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.ui.input_cursor == 0 {
                        Some(Action::MoveSuggestionUp)
                    } else {
                        Some(Action::PreviousInputField)
                    }
                }
                KeyCode::Char(c) => Some(Action::InputPopupChar(c)),
                KeyCode::Backspace => Some(Action::InputPopupBackspace),
                KeyCode::Enter => Some(Action::InputPopupSubmit),
                _ => None,
            };
        }

        return match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Tab => Some(Action::NextTab),
            KeyCode::BackTab => Some(Action::PreviousTab),
            KeyCode::Char('1') => Some(Action::SelectTab(1)),
            KeyCode::Char('2') => Some(Action::SelectTab(2)),
            KeyCode::Char('3') => Some(Action::SelectTab(3)),
            KeyCode::Char('4') => Some(Action::SelectTab(4)),
            KeyCode::Char('5') => Some(Action::SelectTab(5)),
            KeyCode::Char('a') if app.ui.selected_index == 2 || (app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1) => Some(Action::OpenAddPackage),
            KeyCode::Char('a') if app.ui.selected_index == 3 => Some(Action::OpenAddInput),
            KeyCode::Char('s') if app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1 => Some(Action::StartShell(app.shell_packages.clone())),
            KeyCode::Char('x') if app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1 => {
                if let Some(i) = app.ui.shell_package_list_state.selected() {
                    Some(Action::RemovePackage(i))
                } else {
                    None
                }
            }
            KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
            _ => None,
        };
    }
    None
}
