use crate::app::App;
use crate::ui::widgets::{highlight, mark_marker, panel, ruled_header, scrollbar};
use ratatui::{prelude::*, widgets::*};

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let block = panel(" Shell: Packages ", true).border_style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let area = {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    };
    let shell = &mut app.shell;

    if shell.packages.is_empty() {
        let [_, middle, _] = Layout::vertical([
            Constraint::Percentage(45),
            Constraint::Min(3),
            Constraint::Percentage(45),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw("No packages added yet."),
                Line::raw(""),
                Line::raw("Press 'a' to add packages to your temporary shell."),
            ])
            .alignment(Alignment::Center),
            middle,
        );
        return;
    }

    let any_marked = !shell.marked.is_empty();
    let rows: Vec<Row> = shell
        .packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let r = &pkg.reference;
            Row::new(vec![
                Cell::from(mark_marker(any_marked, shell.marked.contains(&i))),
                Cell::from(format!(" {}", r.attr))
                    .style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from("│"),
                Cell::from(format!(" {}", r.version.as_deref().unwrap_or("Unknown")))
                    .style(Style::default().fg(Color::Green)),
                Cell::from("│"),
                Cell::from(format!(" {}", pkg.description)).style(Style::default().fg(Color::Gray)),
                Cell::from("│"),
                Cell::from(format!(" {}", r.rev().unwrap_or("-------")))
                    .style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let len = rows.len();
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Percentage(20),
            Constraint::Length(1),
            Constraint::Percentage(15),
            Constraint::Length(1),
            Constraint::Percentage(35),
            Constraint::Length(1),
            Constraint::Percentage(25),
        ],
    )
    .header(ruled_header(&[
        "",
        " Name",
        "│",
        " Version",
        "│",
        " Description",
        "│",
        " Hash",
    ]))
    .column_spacing(0)
    .row_highlight_style(highlight());
    frame.render_stateful_widget(table, area, &mut shell.table);

    if let Some(progress) = &app.nxv_progress {
        let progress_area = Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(1),
            width: area.width.saturating_sub(4),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Updating NXV Index: ", Style::default().fg(Color::Yellow)),
                Span::raw(progress.clone()),
            ])),
            progress_area,
        );
    }
    scrollbar(frame, area, len, shell.table.selected());
}
