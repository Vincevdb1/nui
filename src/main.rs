use color_eyre::eyre::Result;
use ratatui::{
    crossterm::event::{self, Event},
    widgets::{Paragraph, Widget},
    DefaultTerminal, Frame
};

fn main() -> Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let result = run(terminal);

    ratatui::restore();

    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        // Rendering
        terminal.draw(render)?;

        // Input Handling
        if let Event::Key(key) = event::read()? {
            match key.code {
                event::KeyCode::Char('q') => break,
                _ => {}
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame) {
    Paragraph::new("Test").render(frame.area(), frame.buffer_mut());
}

