use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::app::App;
use crate::components::*;

pub fn render(app: &mut App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    // Create the 2nd row layout (2 columns)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Min(0),
        ])
        .split(chunks[0]);

    // Column 1: 4 rows (Title + 3 even rows)
    let col1_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(body_chunks[0]);

    // Column 2: 2 rows (1st row 67%, 2nd row 33%)
    let col2_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(80),
            Constraint::Percentage(20),
        ])
        .split(body_chunks[1]);

    // Column 1 Components
    title::render(frame, col1_chunks[0], app.selected_index == 0);
    context::render(frame, col1_chunks[1], app.selected_index == 1);
    inputs::render(frame, col1_chunks[2], app.selected_index == 2);
    configurations::render(frame, col1_chunks[3], app.selected_index == 3);

    // Column 2 Components (Content [0] but using index 4 for selection logic if needed, 
    // or keep it 0 but it will be highlighted with Title. Let's use 4 for Content and 5 for Command Log to avoid conflict)
    content::render(frame, col2_chunks[0], app.selected_index == 4);
    command_log::render(frame, col2_chunks[1], app.selected_index == 5);

    let footer = Paragraph::new("Press 'Tab' to switch focus, 'q' to quit")
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(footer, chunks[1]);
}
