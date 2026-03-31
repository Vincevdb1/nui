use crate::app::App;
use crate::nix::flake::extract_packages;
use ratatui::{prelude::*, widgets::*};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut all_packages = Vec::new();

    if let Some(selected_file) = app.nix_files.get(app.selected_nix_file_index) {
        if let Ok(content) = std::fs::read_to_string(&selected_file.path) {
            all_packages.extend(extract_packages(&content));
        }
    }

    if let Some(selected_config) = app.configurations.get(app.selected_configuration_index) {
        if let Some(content) = &selected_config.content {
            all_packages.extend(extract_packages(content));
        }
    }

    let mut seen = std::collections::HashSet::new();
    all_packages.retain(|p| seen.insert(p.name.clone()));

    if all_packages.is_empty() {
        let p = Paragraph::new("No packages found in selected context or configuration.")
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
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
        let (description, version) = if let Some(info) = app.package_info.get(&pkg.name) {
            info.clone()
        } else {
            (pkg.description.clone(), String::new())
        };

        rows.push(Row::new(vec![
            Cell::from(format!(" {}", pkg.name)),
            Cell::from("│"),
            Cell::from(format!(" {}", version)),
            Cell::from("│"),
            Cell::from(format!(" {}", description)),
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
