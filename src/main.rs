mod tui;
mod app;
mod ui;
pub mod components;

use std::io;
use crossterm::event::{self, Event, KeyCode};
use crate::app::App;

fn main() -> io::Result<()> {
    let mut terminal = tui::init()?;
    let mut app = App::new();

    while !app.should_quit {
        // Draw the UI
        terminal.draw(|frame| ui::render(&mut app, frame))?;

        // Handle Events
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.quit(),
                    KeyCode::Char('j') => app.increment(),
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.previous_tab(),
                    _ => {}
                }
            }
        }
        app.tick();
    }

    tui::restore()?;
    Ok(())
}
