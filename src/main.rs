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
        match key.code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Tab => app.next_tab(),
            KeyCode::BackTab => app.previous_tab(),
            _ => {}
        }
    }
    Ok(())
}
