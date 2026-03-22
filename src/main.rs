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
            _ => {}
        }
    }
    Ok(())
}
