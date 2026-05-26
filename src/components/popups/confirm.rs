use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::components::popups::centered_rect;

pub fn render(
    frame: &mut Frame,
    title: &str,
    message: &str,
) {
    let area = centered_rect(50, 25, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" [ {} ] ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let p = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(p, chunks[0]);

    let footer = Paragraph::new("y: Yes / Confirm | n / Esc: No / Cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
