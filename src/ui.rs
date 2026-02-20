use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::app::App;

use crate::components::header;

pub fn render(app: &mut App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    header::render_header(app, frame, chunks[0]);

    let body = Paragraph::new("Press 'j' to increment, 'q' to quit")
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
    frame.render_widget(body, chunks[1]);
}
