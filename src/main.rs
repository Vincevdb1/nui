mod app;
pub mod components;
mod context;
mod nix;
mod tui;
mod ui;

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
                    app.input_cursor = if app.input_cursor == 0 { 2 } else { app.input_cursor - 1 };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            app.suggestions.selected_index = (app.suggestions.selected_index + 1) % app.suggestions.filtered.len();
                            app.suggestions.list_state.select(Some(app.suggestions.selected_index));
                        }
                    } else {
                        app.input_cursor = (app.input_cursor + 1) % 3;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            app.suggestions.selected_index = if app.suggestions.selected_index == 0 {
                                app.suggestions.filtered.len() - 1
                            } else {
                                app.suggestions.selected_index - 1
                            };
                            app.suggestions.list_state.select(Some(app.suggestions.selected_index));
                        }
                    } else {
                        app.input_cursor = if app.input_cursor == 0 { 2 } else { app.input_cursor - 1 };
                    }
                }
                KeyCode::Char(c) => {
                    if app.input_cursor == 1 {
                        app.new_input_name.push(c);
                        app.suggestions.update_filtered(&app.new_input_name);
                    } else if app.input_cursor == 2 {
                        app.new_input_url.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if app.input_cursor == 1 {
                        app.new_input_name.pop();
                        app.suggestions.update_filtered(&app.new_input_name);
                    } else if app.input_cursor == 2 {
                        app.new_input_url.pop();
                    }
                }
                KeyCode::Enter => {
                    if app.input_cursor == 0 {
                        if !app.suggestions.filtered.is_empty() {
                            let (name, url) = &app.suggestions.filtered[app.suggestions.selected_index];
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
            KeyCode::Char('0') => app.selected_index = 0,
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
                app.suggestions.fetch_branches();
            }
            _ => {}
        }
    }
    Ok(())
}
