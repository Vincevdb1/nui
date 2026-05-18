use crate::app::App;
use ratatui::{prelude::*, widgets::*};
use throbber_widgets_tui::Throbber;

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let mut all_packages = Vec::new();

    for (name, (description, version, is_unfree, source_input)) in &app.domain.package_info {
        all_packages.push(crate::nix::Package {
            name: name.clone(),
            description: description.clone(),
            version: Some(version.clone()),
            is_unfree: *is_unfree,
            source_input: if source_input.is_empty() { None } else { Some(source_input.clone()) },
        });
    }

    all_packages.sort_by(|a, b| a.name.cmp(&b.name));

    if all_packages.is_empty() {
        if app.ui.fetching_package_details {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(1),
                    Constraint::Fill(1),
                ])
                .split(area);

            let label = "Fetching package details";
            let label_len = label.len() as u16 + 3; // +3 for the spinner and space

            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(label_len),
                    Constraint::Fill(1),
                ])
                .split(vertical_chunks[1]);

            let throbber = Throbber::default()
                .label(label)
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE);

            frame.render_stateful_widget(
                throbber,
                horizontal_chunks[1],
                &mut app.ui.throbber_state,
            );
        } else {
            let p = Paragraph::new("No packages found in selected configuration.")
                .alignment(Alignment::Center);
            frame.render_widget(p, area);
        }
        return;
    }

    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from(""),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
    ]));

    for pkg in all_packages {
        let is_selected = app.ui.selected_packages.contains(&pkg.name);
        let has_any_selected = !app.ui.selected_packages.is_empty();
        
        let select_marker = if has_any_selected {
            if is_selected {
                Span::styled(" ● ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw(" ○ ")
            }
        } else {
            Span::raw("")
        };

        let unfree_marker = if pkg.is_unfree {
            Span::styled(" $", Style::default().fg(Color::Green))
        } else {
            Span::raw("")
        };

        let current_version = pkg.version.clone().unwrap_or_default();
        let latest = app.domain.package_updates.get(&pkg.name)
            .or_else(|| {
                // Fuzzy match: if we have an update for "jetbrains.datagrip" and we are "datagrip"
                app.domain.package_updates.iter()
                    .find(|(attr, _)| attr.ends_with(&format!(".{}", pkg.name)))
                    .map(|(_, v)| v)
            });

        let version_line = if let Some(latest) = latest {
            if latest != &current_version {
                Line::from(vec![
                    Span::raw(format!(" {}", current_version)),
                    Span::styled(format!(" (󰚰 {})", latest), Style::default().fg(Color::Yellow)),
                ])
            } else {
                Line::from(format!(" {}", current_version))
            }
        } else {
            Line::from(format!(" {}", current_version))
        };

        rows.push(Row::new(vec![
            Cell::from(Line::from(vec![select_marker])),
            Cell::from(Line::from(vec![
                Span::raw(format!(" {}", pkg.name)),
                unfree_marker,
            ])),
            Cell::from("│"),
            Cell::from(version_line),
            Cell::from("│"),
            Cell::from(format!(" {}", pkg.description)),
        ]));
    }

    let rows_count = rows.len();
    let widths = [
        Constraint::Length(3),
        Constraint::Percentage(20),
        Constraint::Length(1),
        Constraint::Percentage(25),
        Constraint::Length(1),
        Constraint::Min(10),
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
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(0)
        .row_highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    frame.render_stateful_widget(table, area, &mut app.ui.package_table_state);

    // Render scrollbar
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));

    let mut scrollbar_state = ScrollbarState::new(rows_count)
        .position(app.ui.package_table_state.selected().unwrap_or(0));

    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}
