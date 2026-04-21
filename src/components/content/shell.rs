use crate::app::App;
use ratatui::{prelude::*, widgets::*};

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    if app.shell_packages.is_empty() {
        let p = Paragraph::new(vec![
            Line::from("No packages added yet."),
            Line::from(""),
            Line::from("Press 'a' to add packages to your temporary shell."),
        ])
        .alignment(Alignment::Center);

        // Center the paragraph vertically
        let inner_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Min(3),
                Constraint::Percentage(45),
            ])
            .split(area)[1];

        frame.render_widget(p, inner_area);
        return;
    }

    let mut rows: Vec<Row> = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
    ]));

    for p in &app.shell_packages {
        let mut name = p.clone();
        let mut version = "Unknown".to_string();
        let mut hash = "-------".to_string();

        if p.contains('@') {
            if let Some((rest, v)) = p.rsplit_once('@') {
                version = v.to_string();
                if rest.starts_with("nixpkgs/") && rest.contains('#') {
                    if let Some((prefix, suffix)) = rest.split_once('#') {
                        name = suffix.to_string();
                        if let Some((_, h)) = prefix.split_once('/') {
                            hash = if h.len() > 7 { h[..7].to_string() } else { h.to_string() };
                        }
                    }
                } else {
                    name = rest.to_string();
                }
            }
        } else if p.starts_with("nixpkgs/") && p.contains('#') {
            if let Some((prefix, suffix)) = p.split_once('#') {
                name = suffix.to_string();
                if let Some((_, h)) = prefix.split_once('/') {
                    hash = if h.len() > 7 { h[..7].to_string() } else { h.to_string() };
                }
            }
        }

        rows.push(Row::new(vec![
            Cell::from(format!(" {}", name)).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("│"),
            Cell::from(format!(" {}", version)).style(Style::default().fg(Color::Green)),
            Cell::from("│"),
            Cell::from(format!(" {}", hash)).style(Style::default().fg(Color::Cyan)),
        ]));
    }

    let widths = [
        Constraint::Percentage(40),
        Constraint::Length(1),
        Constraint::Percentage(30),
        Constraint::Length(1),
        Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from(" Name"),
                Cell::from("│"),
                Cell::from(" Version"),
                Cell::from("│"),
                Cell::from(" Hash"),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(0)
        .row_highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(table, area, &mut app.ui.shell_package_list_state);
}
