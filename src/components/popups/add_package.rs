use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(
    frame: &mut Frame,
    query: &str,
    results: &[(String, String)],
    is_searching: bool,
    list_state: &mut ListState,
) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Add Package ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let search_input = Paragraph::new(query).block(
        Block::default()
            .title(" Search Query ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(search_input, chunks[0]);

    let list_title = if is_searching {
        " Searching... "
    } else {
        " Search Results (Enter to add) "
    };

    if results.is_empty() {
        let block = Block::default()
            .title(list_title)
            .borders(Borders::ALL);
        frame.render_widget(block, chunks[1]);

        let area = chunks[1];
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let empty = Paragraph::new("No results found.").alignment(Alignment::Center);
        frame.render_widget(empty, vertical_chunks[1]);
    } else {
        let items: Vec<ListItem> = results
            .iter()
            .map(|(name, description)| {
                let content = if description.is_empty() {
                    format!("  {}", name)
                } else {
                    let desc_trimmed = if description.len() > 60 {
                        format!("{}...", &description[..57])
                    } else {
                        description.to_string()
                    };
                    format!("  {} - {}", name, desc_trimmed)
                };
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(list_title)
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[1], list_state);
    }

    let footer_text = "Type to search | Enter: Add | Esc: Close";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
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
