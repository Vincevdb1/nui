use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::components::popups::centered_rect;

pub fn render(frame: &mut Frame, filename: &str, description: &str, cursor: usize) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Save Shell as Template ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let filename_style = if cursor == 0 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let desc_style = if cursor == 1 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let filename_input = Paragraph::new(filename).block(
        Block::default()
            .title(" Filename")
            .borders(Borders::ALL)
            .border_style(filename_style),
    );
    frame.render_widget(filename_input, chunks[0]);

    let desc_input = Paragraph::new(description).block(
        Block::default()
            .title(" Description ")
            .borders(Borders::ALL)
            .border_style(desc_style),
    );
    frame.render_widget(desc_input, chunks[1]);

    let info_text = "Packages to include: default devShell with current shell packages.";
    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::Gray))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(info, chunks[2]);

    let footer_text = match cursor {
        0 => "Type Filename... | Tab: Next Field | Esc: Close",
        1 => "Type Description... | Enter: Save Template | Tab: Previous Field | Esc: Close",
        _ => "Press 'Esc' to close",
    };

    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}
