use crate::app::{App, Focus};
use crate::features::log;
use crate::flake::{OutputKind, Package};
use crate::ui::widgets::{highlight, mark_marker, panel, ruled_header, scrollbar};
use ratatui::{prelude::*, widgets::*};
use throbber_widgets_tui::Throbber;

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let [sidebar, main] =
        Layout::horizontal([Constraint::Percentage(33), Constraint::Min(0)]).areas(area);
    let [title, context, inputs, outputs] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .areas(sidebar);

    let focus = app.focus;
    frame.render_widget(
        Paragraph::new(" NUI ")
            .style(Style::default().fg(Color::Cyan))
            .block(panel(" [1] ", focus == Focus::Title)),
        title,
    );
    render_files(app, frame, context);
    render_input_names(app, frame, inputs);
    render_outputs(app, frame, outputs);

    if focus == Focus::Log {
        log::render(frame, main, &mut app.log, true, app.nxv_progress.as_deref());
        return;
    }
    let [content, log_area] =
        Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(main);
    render_content(app, frame, content);
    log::render(
        frame,
        log_area,
        &mut app.log,
        false,
        app.nxv_progress.as_deref(),
    );
}

fn render_files(app: &App, frame: &mut Frame, area: Rect) {
    let items = app.flake.files.iter().enumerate().map(|(i, file)| {
        let style = if i == app.flake.file {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(format!("• {}", file.display())).style(style)
    });
    frame.render_widget(
        List::new(items).block(panel(" [2] Context ", app.focus == Focus::Context)),
        area,
    );
}

fn render_input_names(app: &App, frame: &mut Frame, area: Rect) {
    let items = app.flake.inputs.iter().map(|input| {
        ListItem::new(Span::styled(
            format!("• {}", input.name),
            Style::default().add_modifier(Modifier::BOLD),
        ))
    });
    frame.render_widget(
        List::new(items).block(panel(" [3] Inputs ", app.focus == Focus::Inputs)),
        area,
    );
}

fn render_outputs(app: &App, frame: &mut Frame, area: Rect) {
    let flake = &app.flake;
    let items = flake.outputs.iter().enumerate().map(|(i, output)| {
        let selected = i == flake.output;
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let prefix_style = Style::default().fg(Color::DarkGray);
        let prefix_style = if selected {
            prefix_style.add_modifier(Modifier::BOLD)
        } else {
            prefix_style
        };
        let path = match (output.kind, output.path.split_once('.')) {
            (OutputKind::DevShell, Some((system, name))) => format!("{} ({})", name, system),
            _ => output.path.clone(),
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!("[{}] ", output.kind.label()), prefix_style),
            Span::styled(path, style),
        ]))
    });
    let title = format!(" [4] Outputs ({}) ", flake.outputs.len());
    frame.render_widget(
        List::new(items).block(panel(title, app.focus == Focus::Outputs)),
        area,
    );
}

fn render_content(app: &mut App, frame: &mut Frame, area: Rect) {
    let flake = &app.flake;
    let title = match app.focus {
        Focus::Context | Focus::Outputs => format!(
            " Content: {} / {} ",
            flake
                .selected_file()
                .map(|f| f.display().to_string())
                .unwrap_or_else(|| "None".to_string()),
            flake
                .selected_output()
                .map(|o| o.path.as_str())
                .unwrap_or("None"),
        ),
        Focus::Inputs => " Content: Inputs ".to_string(),
        Focus::Title => " Content: Title ".to_string(),
        Focus::Log => " Content ".to_string(),
    };
    let block = panel(title, app.focus == Focus::Title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.focus {
        Focus::Title => render_about(app, frame, inner),
        Focus::Inputs => render_inputs(app, frame, inner),
        _ => render_packages(app, frame, inner),
    }
}

fn render_about(app: &App, frame: &mut Frame, area: Rect) {
    let logo = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::styled(r"             _ ", logo),
        Line::styled(r" _ __  _   _(_)", logo),
        Line::styled(r"| '_ \| | | | |", logo),
        Line::styled(r"| | | | |_| | |", logo),
        Line::from(vec![
            Span::styled(r"|_| |_|\__,_|_|", logo),
            Span::styled(
                format!(" v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from("NUI - Nix User Interface").bold().cyan(),
        Line::from("A terminal interface for managing Nix flakes.").gray(),
        Line::raw(""),
    ];
    for (tool, version) in [
        ("nix-search", &app.tools.nix_search),
        ("nxv", &app.tools.nxv),
    ] {
        if let Some(v) = version {
            lines.push(Line::from(vec![
                Span::raw(format!("{}: ", tool)).gray(),
                Span::raw(v.clone()).dark_gray(),
            ]));
        }
    }
    lines.extend([
        Line::raw(""),
        Line::from("Thank you for using NUI").bold(),
        Line::raw(""),
        Line::from(vec![
            "Github: ".into(),
            "https://github.com/Vincevdb1/nui".underlined(),
        ])
        .fg(Color::DarkGray),
        Line::from(" Vincent Vandebosch")
            .italic()
            .fg(Color::DarkGray),
        Line::from("󰿃 MIT License").italic().fg(Color::DarkGray),
    ]);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn render_inputs(app: &mut App, frame: &mut Frame, area: Rect) {
    let rows = app.flake.inputs.iter().map(|input| {
        Row::new(vec![
            Cell::from(format!(" {}", input.name)),
            Cell::from("│"),
            Cell::from(format!(" {}", input.url)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Length(1),
            Constraint::Min(10),
        ],
    )
    .header(ruled_header(&[" Name", "│", " URL"]))
    .column_spacing(0)
    .row_highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));
    frame.render_stateful_widget(table, area, &mut app.flake.input_table);
}

fn version_line(pkg: &Package, latest: Option<&String>) -> Line<'static> {
    match latest.filter(|l| **l != pkg.version) {
        Some(latest) => Line::from(vec![
            Span::raw(format!(" {}", pkg.version)),
            Span::styled(
                format!(" (󰚰 {})", latest),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        None => Line::raw(format!(" {}", pkg.version)),
    }
}

fn render_packages(app: &mut App, frame: &mut Frame, area: Rect) {
    let flake = &mut app.flake;
    if flake.packages.is_empty() {
        if flake.loading {
            let label = "Fetching package details";
            let [_, row, _] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);
            let [_, cell, _] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(label.len() as u16 + 3),
                Constraint::Fill(1),
            ])
            .areas(row);
            let throbber = Throbber::default()
                .label(label)
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE);
            frame.render_stateful_widget(throbber, cell, &mut app.throbber);
        } else {
            frame.render_widget(
                Paragraph::new("No packages found in selected configuration.")
                    .alignment(Alignment::Center),
                area,
            );
        }
        return;
    }

    let any_marked = !flake.marked.is_empty();
    let rows: Vec<Row> = flake
        .packages
        .iter()
        .map(|pkg| {
            let latest = flake.updates.get(&pkg.name).or_else(|| {
                flake
                    .updates
                    .iter()
                    .find(|(attr, _)| attr.ends_with(&format!(".{}", pkg.name)))
                    .map(|(_, v)| v)
            });
            let mut name = vec![Span::raw(format!(" {}", pkg.name))];
            if pkg.is_unfree {
                name.push(Span::styled(" $", Style::default().fg(Color::Green)));
            }
            if pkg.is_pinned() {
                name.push(Span::styled(" 󰐃", Style::default().fg(Color::Cyan)));
            }
            Row::new(vec![
                Cell::from(mark_marker(any_marked, flake.marked.contains(&pkg.name))),
                Cell::from(Line::from(name)),
                Cell::from("│"),
                Cell::from(version_line(pkg, latest)),
                Cell::from("│"),
                Cell::from(format!(" {}", pkg.description)),
            ])
        })
        .collect();

    let longest_name = flake
        .packages
        .iter()
        .map(|p| p.name.len() + if p.is_unfree { 2 } else { 0 })
        .max()
        .unwrap_or(20) as u16;
    let name_width = longest_name.clamp(area.width / 5, area.width / 2);

    let len = rows.len();
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(name_width),
            Constraint::Length(1),
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Min(10),
        ],
    )
    .header(ruled_header(&[
        "",
        " Name",
        "│",
        " Version",
        "│",
        " Description",
    ]))
    .column_spacing(0)
    .row_highlight_style(highlight().remove_modifier(Modifier::BOLD));
    frame.render_stateful_widget(table, area, &mut flake.package_table);
    scrollbar(frame, area, len, flake.package_table.selected());
}
