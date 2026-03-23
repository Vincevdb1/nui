use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(
    frame: &mut Frame,
    name: &str,
    url: &str,
    cursor: usize,
) {
    let area = centered_rect(80, 40, frame.area());
    frame.render_widget(Clear, area); //this clears out the background

    let block = Block::default()
        .title(" [ Add Input ] ")
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
        ])
        .split(area);

    let name_style = if cursor == 0 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let url_style = if cursor == 1 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let name_input = Paragraph::new(name)
        .block(Block::default().title(" Name ").borders(Borders::ALL).border_style(name_style));
    frame.render_widget(name_input, chunks[0]);

    let url_input = Paragraph::new(url)
        .block(Block::default().title(" URL ").borders(Borders::ALL).border_style(url_style));
    frame.render_widget(url_input, chunks[1]);

    let footer = Paragraph::new("Press 'Enter' to confirm, 'Esc' to cancel, 'Tab' to switch fields")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
