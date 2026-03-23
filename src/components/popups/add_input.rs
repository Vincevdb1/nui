use crate::nix::suggestions::Suggestions;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, List, ListItem},
};

pub fn render(
    frame: &mut Frame,
    name: &str,
    url: &str,
    cursor: usize,
    suggestions: &mut Suggestions,
) {
    let area = centered_rect(80, 70, frame.area());
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
            Constraint::Min(0),      // Suggestions
            Constraint::Length(10),  // Manual Input block
            Constraint::Length(1),   // Footer
        ])
        .split(area);

    // 1. Suggestions List (Top)
    let list_block_style = if cursor == 0 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let list_title = if suggestions.is_loading {
        " Suggestions (Loading...) "
    } else {
        " Suggestions (Enter to add) "
    };

    if suggestions.is_loading {
        let block = Block::default()
            .title(list_title)
            .borders(Borders::ALL)
            .border_style(list_block_style);
        frame.render_widget(block, chunks[0]);

        let area = chunks[0];
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let loading = Paragraph::new("Loading branches from GitHub...")
            .alignment(Alignment::Center);
        frame.render_widget(loading, vertical_chunks[1]);
    } else if suggestions.filtered.is_empty() {
        let block = Block::default()
            .title(list_title)
            .borders(Borders::ALL)
            .border_style(list_block_style);
        frame.render_widget(block, chunks[0]);

        let area = chunks[0];
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let empty = Paragraph::new("No suggestions found.")
            .alignment(Alignment::Center);
        frame.render_widget(empty, vertical_chunks[1]);
    } else {
        let items: Vec<ListItem> = suggestions.filtered
            .iter()
            .map(|(name, _)| {
                ListItem::new(format!("  {}", name))
            })
            .collect();

        let highlight_style = if cursor == 0 {
            Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::DarkGray)
        };

        let list = List::new(items)
            .block(Block::default().title(list_title).borders(Borders::ALL).border_style(list_block_style))
            .highlight_style(highlight_style)
            .highlight_symbol(">> ");
        
        frame.render_stateful_widget(list, chunks[0], &mut suggestions.list_state);
    }

    // 2. Manual Input Block (Bottom)
    let manual_block = Block::default()
        .title(" Manual Input ")
        .borders(Borders::ALL)
        .border_style(if cursor == 1 || cursor == 2 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        });
    
    let manual_area = chunks[1];
    frame.render_widget(manual_block, manual_area);

    let manual_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(manual_area);

    let name_style = if cursor == 1 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let url_style = if cursor == 2 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let name_input = Paragraph::new(name)
        .block(Block::default().title(" Name ").borders(Borders::ALL).border_style(name_style));
    frame.render_widget(name_input, manual_chunks[0]);

    let url_input = Paragraph::new(url)
        .block(Block::default().title(" URL ").borders(Borders::ALL).border_style(url_style));
    frame.render_widget(url_input, manual_chunks[1]);

    // 3. Footer
    let footer = Paragraph::new("Press 'Enter' to confirm/select, 'Esc' to cancel, 'Tab' to switch fields")
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
