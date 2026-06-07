use crate::state::domain::SearchResult;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};
use std::collections::HashMap;

pub fn render_package_search(
    frame: &mut Frame,
    area: Rect,
    is_searching: bool,
    error: Option<&str>,
    results: &[SearchResult],
    channels: &[String],
    list_state: &mut ListState,
    installed_packages: &HashMap<String, (String, String, bool, String)>,
    is_shell_mode: bool,
    system_nixpkgs_version: Option<&String>,
    system_nixpkgs_hash: Option<&String>,
) {
    let list_title = if is_searching {
        " Searching... ".to_string()
    } else {
        " Search Results (Enter to select) ".to_string()
    };

    if results.is_empty() {
        let block = Block::default().title(list_title).borders(Borders::ALL);
        frame.render_widget(block, area);

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(if error.is_some() { 4 } else { 1 }),
                Constraint::Percentage(50),
            ])
            .split(area);

        if let Some(err) = error {
            let error_text = vec![
                Line::from(vec![Span::styled(
                    "Error searching packages:",
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
            let empty = Paragraph::new("No results found.").alignment(Alignment::Center);
            frame.render_widget(empty, vertical_chunks[1]);
        }
    } else {
        // Calculate widths for each column
        let max_name_width = results
            .iter()
            .map(|res| res.name.len())
            .max()
            .unwrap_or(0)
            .max(10)
            + 2; // +2 for unfree marker space

        if is_shell_mode {
            let sys_v = system_nixpkgs_version.map(|v| v.as_str());
            let max_version_width = results
                .iter()
                .map(|res| {
                    let version = res
                        .versions
                        .iter()
                        .find(|v| v.channel == "system")
                        .map(|v| v.version.as_str())
                        .unwrap_or_else(|| {
                            if let Some(sys_v) = sys_v {
                                res.versions
                                    .iter()
                                    .find(|v| v.channel == sys_v || sys_v.starts_with(&v.channel))
                                    .map(|v| v.version.as_str())
                                    .unwrap_or_else(|| {
                                        res.versions
                                            .first()
                                            .map(|v| v.version.as_str())
                                            .unwrap_or("Unknown")
                                    })
                            } else {
                                res.versions
                                    .first()
                                    .map(|v| v.version.as_str())
                                    .unwrap_or("Unknown")
                            }
                        });
                    let mut len = version.len();
                    let sys_hash = system_nixpkgs_hash.map(|s| s.as_str());
                    if res.hash.as_deref().unwrap_or("") != sys_hash.unwrap_or("") {
                        len += 2; // " 󰚰"
                    }
                    len
                })
                .max()
                .unwrap_or(0)
                .max(10);

            // Header
            let header_spans = vec![
                Span::raw("      "), // 1 for border + 3 for selection + 2 for unfree marker space
                Span::styled(
                    format!("{:width$}", "Name", width = max_name_width - 1),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled(
                    format!("{:width$}", "Version", width = max_version_width),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled("Description", Style::default().add_modifier(Modifier::BOLD)),
            ];
            let header = Paragraph::new(Line::from(header_spans));

            let items: Vec<ListItem> = results
                .iter()
                .map(|res| {
                    let unfree_marker = if res.is_unfree {
                        Span::styled("$ ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("  ")
                    };

                    let mut spans = vec![
                        unfree_marker,
                        Span::styled(
                            format!("{:width$}", res.name, width = max_name_width - 1),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];

                    spans.push(Span::raw(" | "));
                    let mut version = res
                        .versions
                        .iter()
                        .find(|v| v.channel == "system")
                        .map(|v| v.version.clone())
                        .unwrap_or_else(|| {
                            if let Some(sys_v) = sys_v {
                                res.versions
                                    .iter()
                                    .find(|v| v.channel == sys_v || sys_v.starts_with(&v.channel))
                                    .map(|v| v.version.clone())
                                    .unwrap_or_else(|| {
                                        res.versions
                                            .first()
                                            .map(|v| v.version.clone())
                                            .unwrap_or_else(|| "Unknown".to_string())
                                    })
                            } else {
                                res.versions
                                    .first()
                                    .map(|v| v.version.clone())
                                    .unwrap_or_else(|| "Unknown".to_string())
                            }
                        });
                    let sys_hash = system_nixpkgs_hash.map(|s| s.as_str());
                    let has_update = res.hash.as_deref().unwrap_or("") != sys_hash.unwrap_or("");
                    if has_update {
                        version.push_str(" 󰚰");
                    }
                    spans.push(Span::styled(
                        format!("{:width$}", version, width = max_version_width),
                        Style::default().fg(Color::Green),
                    ));

                    spans.push(Span::raw(" | "));
                    if !res.description.is_empty() {
                        let desc_trimmed = if res.description.chars().count() > 60 {
                            let truncated: String = res.description.chars().take(57).collect();
                            format!("{}...", truncated)
                        } else {
                            res.description.to_string()
                        };
                        spans.push(Span::styled(
                            desc_trimmed,
                            Style::default().fg(Color::DarkGray),
                        ));
                    }

                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(area);

            frame.render_widget(header, list_chunks[0]);

            let list = List::new(items)
                .block(Block::default().title(list_title).borders(Borders::ALL))
                .highlight_style(
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_stateful_widget(list, list_chunks[1], list_state);

            // Render scrollbar
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"));

            let mut scrollbar_state =
                ScrollbarState::new(results.len()).position(list_state.selected().unwrap_or(0));

            frame.render_stateful_widget(
                scrollbar,
                list_chunks[1].inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        } else {
            let mut max_version_widths = Vec::new();
            for channel in channels {
                let max_w = results
                    .iter()
                    .map(|res| {
                        res.versions
                            .iter()
                            .find(|v| v.channel == *channel)
                            .map(|v| {
                                let mut w = v.version.len();
                                if let Some(lv) = &v.locked_version {
                                    w += lv.len() + 3; // " (v)"
                                }
                                w + 4 // +4 for (󰚰 )
                            })
                            .unwrap_or(0)
                    })
                    .max()
                    .unwrap_or(0);
                max_version_widths.push(max_w.max(channel.len()).max(5));
            }

            // Header needs 3 spaces padding to account for the List's highlight symbol ">> "
            // plus 1 extra for the List's block border
            let mut header_spans = vec![
                Span::raw("      "), // 1 for border + 3 for selection + 2 for unfree marker space
                Span::styled(
                    format!("{:width$}", "Name", width = max_name_width - 1),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];

            for (i, channel) in channels.iter().enumerate() {
                header_spans.push(Span::raw(" | "));
                let color = get_channel_color(channel);
                header_spans.push(Span::styled(
                    format!("{:width$}", channel, width = max_version_widths[i]),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
            header_spans.push(Span::raw(" | "));
            header_spans.push(Span::styled(
                "Description",
                Style::default().add_modifier(Modifier::BOLD),
            ));

            let header = Paragraph::new(Line::from(header_spans));

            let items: Vec<ListItem> = results
                .iter()
                .map(|res| {
                    let unfree_marker = if res.is_unfree {
                        Span::styled("$ ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("  ")
                    };

                    // Find installed version
                    let installed_pkg = installed_packages.get(&res.name).or_else(|| {
                        // Fuzzy match installed packages
                        installed_packages
                            .iter()
                            .find(|(name, _)| res.name.ends_with(&format!(".{}", name)))
                            .map(|(_, v)| v)
                    });

                    let installed_version = installed_pkg.map(|(_, ver, _, _)| ver.as_str());

                    let mut spans = vec![
                        unfree_marker,
                        Span::styled(
                            format!("{:width$}", res.name, width = max_name_width - 1),
                            if installed_version.is_some() {
                                Style::default()
                                    .add_modifier(Modifier::BOLD)
                                    .fg(Color::Cyan)
                            } else {
                                Style::default().add_modifier(Modifier::BOLD)
                            },
                        ),
                    ];

                    for (i, channel) in channels.iter().enumerate() {
                        spans.push(Span::raw(" | "));
                        let version_opt = res.versions.iter().find(|v| v.channel == *channel);
                        let width = max_version_widths[i];

                        if let Some(v) = version_opt {
                            let color = get_channel_color(channel);

                            let version_str = if let Some(lv) = &v.locked_version {
                                if lv != &v.version {
                                    format!("{} (󰌾 {})", v.version, lv)
                                } else {
                                    v.version.clone()
                                }
                            } else {
                                v.version.clone()
                            };

                            spans.push(Span::styled(
                                format!("{:width$}", version_str, width = width),
                                Style::default().fg(color),
                            ));
                        } else {
                            spans.push(Span::raw(" ".repeat(width)));
                        }
                    }

                    spans.push(Span::raw(" | "));

                    if !res.description.is_empty() {
                        let desc_trimmed = if res.description.chars().count() > 60 {
                            let truncated: String = res.description.chars().take(57).collect();
                            format!("{}...", truncated)
                        } else {
                            res.description.to_string()
                        };
                        spans.push(Span::styled(
                            desc_trimmed,
                            Style::default().fg(Color::DarkGray),
                        ));
                    }

                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(area);

            frame.render_widget(header, list_chunks[0]);

            let list = List::new(items)
                .block(Block::default().title(list_title).borders(Borders::ALL))
                .highlight_style(
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_stateful_widget(list, list_chunks[1], list_state);

            // Render scrollbar
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"));

            let mut scrollbar_state =
                ScrollbarState::new(results.len()).position(list_state.selected().unwrap_or(0));

            frame.render_stateful_widget(
                scrollbar,
                list_chunks[1].inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }
}

fn get_channel_color(channel: &str) -> Color {
    if channel == "system" {
        return Color::Cyan;
    }
    let mut hash: u32 = 0;
    for c in channel.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as u32);
    }

    let colors = [
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
    ];
    colors[(hash as usize) % colors.len()]
}
