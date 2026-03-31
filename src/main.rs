mod app;
pub mod components;
mod context;
mod nix;
mod tui;
mod ui;

pub use components::command_log::command_log;

use crate::app::App;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = tui::init()?;
    let mut app = App::new();

    let result = run(&mut terminal, &mut app);

    tui::restore()?;
    result
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::render(app, frame))?;

        if event::poll(std::time::Duration::from_millis(16))? {
            handle_events(app, event::read()?)?;
        }
        app.tick();
    }
    Ok(())
}

fn handle_events(app: &mut App, event: Event) -> Result<()> {
    if let Event::Key(key) = event {
        if app.is_adding_input {
            match key.code {
                KeyCode::Esc => {
                    app.is_adding_input = false;
                    app.new_input_name.clear();
                    app.new_input_url.clear();
                    app.input_cursor = 0;
                }
                KeyCode::Tab => {
                    app.input_cursor = (app.input_cursor + 1) % 3;
                }
                KeyCode::BackTab => {
                    app.input_cursor = if app.input_cursor == 0 {
                        2
                    } else {
                        app.input_cursor - 1
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            app.suggestions.selected_index = (app.suggestions.selected_index + 1)
                                % app.suggestions.filtered.len();
                            app.suggestions
                                .list_state
                                .select(Some(app.suggestions.selected_index));
                        }
                    } else {
                        app.input_cursor = (app.input_cursor + 1) % 3;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            app.suggestions.selected_index = if app.suggestions.selected_index == 0
                            {
                                app.suggestions.filtered.len() - 1
                            } else {
                                app.suggestions.selected_index - 1
                            };
                            app.suggestions
                                .list_state
                                .select(Some(app.suggestions.selected_index));
                        }
                    } else {
                        app.input_cursor = if app.input_cursor == 0 {
                            2
                        } else {
                            app.input_cursor - 1
                        };
                    }
                }
                KeyCode::Char(c) => {
                    if app.input_cursor == 1 {
                        app.new_input_name.push(c);
                        app.update_suggestions();
                    } else if app.input_cursor == 2 {
                        app.new_input_url.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if app.input_cursor == 1 {
                        app.new_input_name.pop();
                        app.update_suggestions();
                    } else if app.input_cursor == 2 {
                        app.new_input_url.pop();
                    }
                }
                KeyCode::Enter => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            let (name, url) =
                                &app.suggestions.filtered[app.suggestions.selected_index];
                            app.new_input_name = name.clone();
                            app.new_input_url = url.clone();
                            app.add_input();
                        }
                    } else if !app.new_input_name.is_empty() && !app.new_input_url.is_empty() {
                        app.add_input();
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Tab => app.next_tab(),
            KeyCode::BackTab => app.previous_tab(),
            KeyCode::Char('1') => app.selected_index = 1,
            KeyCode::Char('2') => app.selected_index = 2,
            KeyCode::Char('3') => app.selected_index = 3,
            KeyCode::Char('4') => app.selected_index = 4,
            KeyCode::Char('5') => app.selected_index = 5,
            KeyCode::Char('a') if app.selected_index == 3 => {
                app.is_adding_input = true;
                app.new_input_name.clear();
                app.new_input_url.clear();
                app.input_cursor = 0;
                app.update_suggestions();
                app.start_fetching();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.selected_index == 2 {
                    if !app.nix_files.is_empty() {
                        app.selected_nix_file_index =
                            (app.selected_nix_file_index + 1) % app.nix_files.len();
                        app.update_context();
                    }
                } else if app.selected_index == 4 {
                    if !app.configurations.is_empty() {
                        app.selected_configuration_index =
                            (app.selected_configuration_index + 1) % app.configurations.len();
                        app.fetch_package_details_from_config();
                    }
                } else if app.selected_index == 5 {
                    if !app.logs.is_empty() {
                        let i = match app.command_log_state.selected() {
                            Some(i) => {
                                if i >= app.logs.len() - 1 {
                                    i
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        app.command_log_state.select(Some(i));
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.selected_index == 2 {
                    if !app.nix_files.is_empty() {
                        app.selected_nix_file_index = if app.selected_nix_file_index == 0 {
                            app.nix_files.len() - 1
                        } else {
                            app.selected_nix_file_index - 1
                        };
                        app.update_context();
                    }
                } else if app.selected_index == 4 {
                    if !app.configurations.is_empty() {
                        app.selected_configuration_index = if app.selected_configuration_index == 0
                        {
                            app.configurations.len() - 1
                        } else {
                            app.selected_configuration_index - 1
                        };
                        app.fetch_package_details_from_config();
                    }
                } else if app.selected_index == 5 {
                    if !app.logs.is_empty() {
                        let i = match app.command_log_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    0
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        app.command_log_state.select(Some(i));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
