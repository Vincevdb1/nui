use crate::app::App;
use ratatui::{prelude::*, widgets::*};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut all_packages = Vec::new();

    for (name, (description, version)) in &app.package_info {
        all_packages.push(crate::nix::Package {
            name: name.clone(),
            description: description.clone(),
            version: Some(version.clone()),
        });
    }

    all_packages.sort_by(|a, b| a.name.cmp(&b.name));

    if all_packages.is_empty() {
        if app.fetching_package_details {
            let p = Paragraph::new("Fetching package details...")
                .alignment(Alignment::Center);
            frame.render_widget(p, area);
        } else {
            let p = Paragraph::new("No packages found in selected configuration.")
                .alignment(Alignment::Center);
            frame.render_widget(p, area);
        }
        return;
    }

    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
    ]));

    for pkg in all_packages {
        rows.push(Row::new(vec![
            Cell::from(format!(" {}", pkg.name)),
            Cell::from("│"),
            Cell::from(format!(" {}", pkg.version.unwrap_or_default())),
            Cell::from("│"),
            Cell::from(format!(" {}", pkg.description)),
        ]));
    }

    let widths = [
        Constraint::Percentage(20),
        Constraint::Length(1),
        Constraint::Percentage(15),
        Constraint::Length(1),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from(" Name"),
                Cell::from("│"),
                Cell::from(" Version"),
                Cell::from("│"),
                Cell::from(" Description"),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(0);

    frame.render_widget(table, area);
}
