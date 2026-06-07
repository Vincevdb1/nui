use crate::nix::suggestions::Suggestions;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::components::popups::centered_rect;

pub struct AddInputProps<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub cursor: usize,
    pub suggestions: &'a mut Suggestions,
}

pub fn render(frame: &mut Frame, props: &mut AddInputProps) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Add Input ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .split(area);

    let list_block_style = if props.cursor == 0 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let list_title = if props.suggestions.is_loading {
        " Suggestions (Loading...) "
    } else {
        " Suggestions (Enter to add) "
    };

    if props.suggestions.is_loading {
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

        let loading =
            Paragraph::new("Loading branches from GitHub...").alignment(Alignment::Center);
        frame.render_widget(loading, vertical_chunks[1]);
    } else if props.suggestions.filtered.is_empty() {
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
                Constraint::Length(if props.suggestions.error.is_some() {
                    4
                } else {
                    1
                }),
                Constraint::Percentage(50),
            ])
            .split(area);

        if let Some(err) = &props.suggestions.error {
            use ratatui::text::{Line, Span};
            let error_text = vec![
                Line::from(vec![Span::styled(
                    "Error fetching suggestions:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(err, Style::default().fg(Color::Red))]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Please check your internet connection.",
                    Style::default().fg(Color::DarkGray),
                )]),
            ];
            let error_para = Paragraph::new(error_text).alignment(Alignment::Center);
            frame.render_widget(error_para, vertical_chunks[1]);
        } else {
            let empty = Paragraph::new("No suggestions found.").alignment(Alignment::Center);
            frame.render_widget(empty, vertical_chunks[1]);
        }
    } else {
        let items: Vec<ListItem> = props
            .suggestions
            .filtered
            .iter()
            .map(|(name, _)| ListItem::new(format!("  {}", name)))
            .collect();

        let highlight_style = if props.cursor == 0 {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::DarkGray)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title(list_title)
                    .borders(Borders::ALL)
                    .border_style(list_block_style),
            )
            .highlight_style(highlight_style)
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[0], &mut props.suggestions.list_state);
    }

    let manual_block = Block::default()
        .title(" Manual Input ")
        .borders(Borders::ALL)
        .border_style(if props.cursor == 1 || props.cursor == 2 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        });

    let manual_area = chunks[1];
    frame.render_widget(manual_block, manual_area);

    let manual_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(manual_area);

    let name_style = if props.cursor == 1 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let url_style = if props.cursor == 2 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let name_input = Paragraph::new(props.name).block(
        Block::default()
            .title(" Name ")
            .borders(Borders::ALL)
            .border_style(name_style),
    );
    frame.render_widget(name_input, manual_chunks[0]);

    let url_input = Paragraph::new(props.url).block(
        Block::default()
            .title(" URL ")
            .borders(Borders::ALL)
            .border_style(url_style),
    );
    frame.render_widget(url_input, manual_chunks[1]);

    let footer_text = match props.cursor {
        0 => "Enter: Select | j/k: Nav Suggestions | Tab: Next Field | Esc: Close",
        1 => "Type Input Name... | Tab: Next Field | Esc: Close",
        2 => "Type Input URL... | Enter: Add | Tab: Next Field | Esc: Close",
        _ => "Press 'Esc' to close",
    };

    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
