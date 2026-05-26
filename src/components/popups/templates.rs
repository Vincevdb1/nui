use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState, Paragraph},
};

use crate::components::popups::centered_rect;

pub struct TemplatesProps<'a> {
    pub table_state: &'a mut TableState,
    pub templates: &'a [(String, String)],
}

pub fn render(frame: &mut Frame, props: &mut TemplatesProps) {
    let area = centered_rect(80, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Select Template ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    if props.templates.is_empty() {
        let empty = Paragraph::new("No templates found in ~/.config/nui/templates/")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, chunks[0]);
    } else {
        let mut rows = Vec::new();

        // Header
        rows.push(Row::new(vec![
            Cell::from(Span::styled(" Template Name", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
            Cell::from(""),
            Cell::from(Span::styled(" Description", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        ]));

        // Separator
        rows.push(Row::new(vec![
            Cell::from("─".repeat(100)),
            Cell::from("┼"),
            Cell::from("─".repeat(100)),
        ]));

        for (t, d) in props.templates {
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", t)),
                Cell::from("│"),
                Cell::from(format!(" {}", d)),
            ]));
        }

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(30),
                Constraint::Length(1),
                Constraint::Min(20),
            ],
        )
        .block(Block::default().borders(Borders::NONE))
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(50, 50, 50))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        frame.render_stateful_widget(table, chunks[0], props.table_state);
    }

    let footer = Paragraph::new("Enter: Use Template | Esc: Skip / Close")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
