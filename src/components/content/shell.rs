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
        Cell::from(""),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
    ]));

    for p in &app.shell_packages {
        let is_selected = app.ui.selected_shell_packages.contains(p);
        let has_any_selected = !app.ui.selected_shell_packages.is_empty();
        
        let select_marker = if has_any_selected {
            if is_selected {
                Span::styled(" ● ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw(" ○ ")
            }
        } else {
            Span::raw("")
        };

        let mut display_name = p.clone();
        let mut version = "Unknown".to_string();
        let mut hash = "-------".to_string();
        let mut description = String::new();

        // 1. Try lookup in package_info first (for metadata from templates or direct adds)
        if let Some((desc, ver, _, _)) = app.domain.package_info.get(p) {
            version = ver.clone();
            description = desc.clone();
        } else {
            // 2. Fallback to parsing the string if it contains extra info
            if p.contains('@') {
                if let Some((rest, v)) = p.rsplit_once('@') {
                    version = v.to_string();
                    if (rest.starts_with("nixpkgs/") || rest.starts_with("system/")) && rest.contains('#') {
                        if let Some((prefix, suffix)) = rest.split_once('#') {
                            display_name = suffix.to_string();
                            if let Some((_, h)) = prefix.split_once('/') {
                                hash = h.to_string();
                            }
                        }
                    } else {
                        display_name = rest.to_string();
                    }
                }
            } else if (p.starts_with("nixpkgs/") || p.starts_with("system/")) && p.contains('#') {
                if let Some((prefix, suffix)) = p.split_once('#') {
                    display_name = suffix.to_string();
                    if let Some((_, h)) = prefix.split_once('/') {
                        hash = h.to_string();
                    }
                }
            }
        }

        rows.push(Row::new(vec![
            Cell::from(Line::from(vec![select_marker])),
            Cell::from(format!(" {}", display_name)).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("│"),
            Cell::from(format!(" {}", version)).style(Style::default().fg(Color::Green)),
            Cell::from("│"),
            Cell::from(format!(" {}", description)).style(Style::default().fg(Color::Gray)),
            Cell::from("│"),
            Cell::from(format!(" {}", hash)).style(Style::default().fg(Color::Cyan)),
        ]));
    }

    let rows_count = rows.len();
    let widths = [
        Constraint::Length(3),
        Constraint::Percentage(20),
        Constraint::Length(1),
        Constraint::Percentage(15),
        Constraint::Length(1),
        Constraint::Percentage(35),
        Constraint::Length(1),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from(""),
                Cell::from(" Name"),
                Cell::from("│"),
                Cell::from(" Version"),
                Cell::from("│"),
                Cell::from(" Description"),
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

    // Render NXV update progress if active
    if let Some(progress) = app.domain.nxv_update_progress.as_deref() {
        let progress_area = Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(1),
            width: area.width.saturating_sub(4),
            height: 1,
        };
        let p = Paragraph::new(Line::from(vec![
            Span::styled(" Updating NXV Index: ", Style::default().fg(Color::Yellow)),
            Span::raw(progress),
        ]));
        frame.render_widget(p, progress_area);
    }

    // Render scrollbar
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));

    let mut scrollbar_state = ScrollbarState::new(rows_count)
        .position(app.ui.shell_package_list_state.selected().unwrap_or(0));

    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}
