mod tui;
mod app;
mod ui;
pub mod components;

use crossterm::event::{self, Event, KeyCode};
use crate::app::App;
use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = tui::init()?;
    let mut app = App::new();

    let result = (|| {
        while !app.should_quit {
            // Draw the UI
            terminal.draw(|frame| ui::render(&mut app, frame))?;

            // Handle Events
            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => app.quit(),
                        KeyCode::Tab => app.next_tab(),
                        KeyCode::BackTab => app.previous_tab(),
                        _ => {}
                    }
                }
            }
            app.tick();
        }
        Ok(())
    })();

    tui::restore()?;
    result
}
