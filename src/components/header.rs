use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use crate::app::App;

pub fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(format!(" Status: RUNNING | Count: {} ", app.counter))
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM).title(" Ratatui App "));

    frame.render_widget(header, area);
}
