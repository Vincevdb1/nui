use crate::state::domain::{SearchResult, VersionInfo};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use std::collections::HashMap;

pub fn render(
    frame: &mut Frame,
    query: &str,
    results: &[SearchResult],
    channels: &[String],
    is_searching: bool,
    list_state: &mut ListState,
    is_selecting_version: bool,
    is_fetching_versions: bool,
    versions: &[VersionInfo],
    version_list_state: &mut ListState,
    installed_packages: &HashMap<String, (String, String, bool, String)>,
    is_shell_mode: bool,
) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Add Package ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let search_input = Paragraph::new(query).block(
        Block::default()
            .title(" Search Query ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(search_input, chunks[0]);

    if is_selecting_version {
        render_version_selection(
            frame,
            chunks[1],
            is_fetching_versions,
            versions,
            version_list_state,
        );
    } else {
        render_package_search(
            frame,
            chunks[1],
            is_searching,
            results,
            channels,
            list_state,
            installed_packages,
            is_shell_mode,
        );
    }

    let footer_text = if is_selecting_version {
        "Enter: Select Version | Esc: Back to Search | q: Close"
    } else {
        "Type: Search | Enter: Add | M-Enter: Versions | Tab: Details | Esc/q: Close"
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn render_package_search(
    frame: &mut Frame,
    area: Rect,
    is_searching: bool,
    results: &[SearchResult],
    channels: &[String],
    list_state: &mut ListState,
    installed_packages: &HashMap<String, (String, String, bool, String)>,
    is_shell_mode: bool,
) {
    let list_title = if is_searching {
        " Searching... "
    } else {
        " Search Results (Enter to select) "
    };

    if results.is_empty() {
        let block = Block::default().title(list_title).borders(Borders::ALL);
        frame.render_widget(block, area);

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let empty = Paragraph::new("No results found.").alignment(Alignment::Center);
        frame.render_widget(empty, vertical_chunks[1]);
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
            let max_version_width = results
                .iter()
                .map(|res| {
                    res.versions.first().map(|v| v.version.len()).unwrap_or(0)
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
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled(
                    format!("{:8}", "Hash"),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled(
                    "Description",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
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
                    let version = res.versions.first().map(|v| v.version.as_str()).unwrap_or("Unknown");
                    spans.push(Span::styled(
                        format!("{:width$}", version, width = max_version_width),
                        Style::default().fg(Color::Green),
                    ));

                    spans.push(Span::raw(" | "));
                    let hash = res.hash.as_deref().unwrap_or("-------");
                    let short_hash = if hash.len() > 7 { &hash[..7] } else { hash };
                    spans.push(Span::styled(
                        format!("{:8}", short_hash),
                        Style::default().fg(Color::Cyan),
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
                    let installed_pkg = installed_packages.get(&res.name)
                        .or_else(|| {
                            // Fuzzy match installed packages
                            installed_packages.iter()
                                .find(|(name, _)| res.name.ends_with(&format!(".{}", name)))
                                .map(|(_, v)| v)
                        });
                    
                    let installed_version = installed_pkg.map(|(_, ver, _, _)| ver.as_str());

                    let mut spans = vec![
                        unfree_marker,
                        Span::styled(
                            format!("{:width$}", res.name, width = max_name_width - 1),
                            if installed_version.is_some() {
                                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)
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
        }
    }
}

fn render_version_selection(
    frame: &mut Frame,
    area: Rect,
    is_fetching: bool,
    versions: &[VersionInfo],
    list_state: &mut ListState,
) {
    let list_title = if is_fetching {
        " Searching for versions... "
    } else {
        " Select Version (Enter to add) "
    };

    if versions.is_empty() && !is_fetching {
        let block = Block::default().title(list_title).borders(Borders::ALL);
        frame.render_widget(block, area);

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let empty = Paragraph::new("No versions found.").alignment(Alignment::Center);
        frame.render_widget(empty, vertical_chunks[1]);
    } else if is_fetching && versions.is_empty() {
        let block = Block::default().title(list_title).borders(Borders::ALL);
        frame.render_widget(block, area);

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        let loading = Paragraph::new("Fetching versions from nxv...").alignment(Alignment::Center);
        frame.render_widget(loading, vertical_chunks[1]);
    } else {
        let header_spans = vec![
            Span::raw("      "), // 1 for border + 3 for selection + 2 for unfree marker space
            Span::styled(
                format!("{:16}", "Version"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                format!("{:10}", "Hash"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                "Date",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let header = Paragraph::new(Line::from(header_spans));

        let items: Vec<ListItem> = versions
            .iter()
            .map(|v| {
                let unfree_marker = if v.is_unfree {
                    Span::styled("$ ", Style::default().fg(Color::Green))
                } else {
                    Span::raw("  ")
                };

                let spans = vec![
                    unfree_marker,
                    Span::styled(
                        format!("{:16}", v.version),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(format!("{:10}", v.hash), Style::default().fg(Color::Cyan)),
                    Span::raw(" | "),
                    Span::styled(&v.date, Style::default().fg(Color::DarkGray)),
                ];
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
    }
}

pub fn render_details(frame: &mut Frame, result: &SearchResult) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" [ {} Details ] ", result.name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Calculate height needed for platforms
    let platforms_text = result.platforms.join(", ");
    let platforms_height = if result.platforms.is_empty() {
        0
    } else {
        let text_len = 11 + platforms_text.len(); // "Platforms: " is 11 chars
        let width = inner_area.width.saturating_sub(2) as usize; // Account for margins
        if width == 0 {
            1
        } else {
            text_len.div_ceil(width) as u16
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),                                // Name
            Constraint::Length(if result.source_input.is_some() { 1 } else { 0 }), // Source
            Constraint::Length(result.versions.len() as u16 + 1), // Versions
            Constraint::Length(platforms_height),                 // Platforms
            Constraint::Min(0),                                   // Description
            Constraint::Length(1),                                // Footer
        ])
        .split(inner_area);

    // Name
    let unfree_marker = if result.is_unfree {
        Span::styled(" [UNFREE]", Style::default().fg(Color::Green))
    } else {
        Span::raw("")
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Attribute: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&result.name, Style::default().add_modifier(Modifier::BOLD)),
            unfree_marker,
        ])),
        chunks[0],
    );

    // Source
    if let Some(source) = &result.source_input {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
                Span::styled(source, Style::default().fg(Color::Yellow)),
            ])),
            chunks[1],
        );
    }

    // Versions
    let mut version_lines = vec![Line::from(Span::styled(
        "Available Versions:",
        Style::default().fg(Color::DarkGray),
    ))];
    for ver in &result.versions {
        let color = get_channel_color(&ver.channel);
        version_lines.push(Line::from(vec![
            Span::raw("  • "),
            Span::styled(&ver.channel, Style::default().fg(color)),
            Span::raw(": "),
            Span::styled(&ver.version, Style::default().add_modifier(Modifier::BOLD)),
        ]));
    }
    frame.render_widget(Paragraph::new(version_lines), chunks[2]);

    // Platforms
    if !result.platforms.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Platforms: ", Style::default().fg(Color::DarkGray)),
                Span::raw(platforms_text),
            ]))
            .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[3],
        );
    }

    // Description
    let desc = if result.description.is_empty() {
        "No description available."
    } else {
        &result.description
    };
    frame.render_widget(
        Paragraph::new(desc)
            .block(
                Block::default()
                    .title(" Description ")
                    .borders(Borders::TOP),
            )
            .wrap(ratatui::widgets::Wrap { trim: true }),
        chunks[4],
    );

    // Footer
    frame.render_widget(
        Paragraph::new("Press Tab, q, or Esc to close")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn get_channel_color(channel: &str) -> Color {
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
