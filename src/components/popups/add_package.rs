use crate::state::domain::{SearchResult, VersionInfo, ChannelVersion};
use crate::nix::Input;
use crate::state::Mode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::components::popups::centered_rect;

use std::collections::HashMap;

pub struct AddPackageProps<'a> {
    pub query: &'a str,
    pub results: &'a [SearchResult],
    pub channels: &'a [String],
    pub is_searching: bool,
    pub error: Option<&'a str>,
    pub list_state: &'a mut ListState,
    pub is_selecting_version: bool,
    pub is_fetching_versions: bool,
    pub versions: &'a [VersionInfo],
    pub version_fetch_error: Option<&'a str>,
    pub version_list_state: &'a mut ListState,
    pub installed_packages: &'a HashMap<String, (String, String, bool, String)>,
    pub is_shell_mode: bool,
    pub inputs: &'a [Input],
    pub selected_package_name: Option<&'a String>,
    pub mode: Mode,
    pub system_nixpkgs_version: Option<&'a String>,
    pub system_nixpkgs_hash: Option<&'a String>,
    pub is_swapping: bool,
}

pub fn render(
    frame: &mut Frame,
    props: &mut AddPackageProps,
) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let title = if props.is_swapping {
        " [ Change Version ] "
    } else {
        " [ Add Package ] "
    };

    let block = Block::default()
        .title(title)
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

    let search_input = Paragraph::new(props.query).block(
        Block::default()
            .title(" Search Query ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(search_input, chunks[0]);

    if props.is_selecting_version {
        let current_versions = if let Some(pkg_name) = props.selected_package_name {
            props.results.iter()
                .find(|res| &res.name == pkg_name)
                .map(|res| res.versions.as_slice())
                .unwrap_or(&[])
        } else {
            &[]
        };

        render_version_selection(
            frame,
            chunks[1],
            props.is_fetching_versions,
            props.version_fetch_error,
            props.versions,
            props.version_list_state,
            props.inputs,
            current_versions,
            props.mode.clone(),
        );
    } else {
        render_package_search(
            frame,
            chunks[1],
            props.is_searching,
            props.error,
            props.results,
            props.channels,
            props.list_state,
            props.installed_packages,
            props.is_shell_mode,
            props.system_nixpkgs_version,
            props.system_nixpkgs_hash,
        );
    }

    let footer_text = if props.is_selecting_version {
        "Enter: Select Version | Esc: Back to Search"
    } else {
        "Type: Search | Enter: Add | M-Enter: Versions | Tab: Details | Esc: Close"
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn render_package_search(
    frame: &mut Frame,
    area: Rect,
    is_searching: bool,
    error: Option<&str>,
    results: &[SearchResult],
    channels: &[String],
    list_state: &mut ListState,
    installed_packages: &HashMap<String, (String, String, bool, String)>,
    is_shell_mode: bool,
    _system_nixpkgs_version: Option<&String>,
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
                Line::from(vec![Span::styled("Error searching packages:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))]),
                Line::from(vec![Span::styled(err, Style::default().fg(Color::Red))]),
                Line::from(""),
                Line::from(vec![Span::styled("Please check your internet connection.", Style::default().fg(Color::DarkGray))]),
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
            let max_version_width = results
                .iter()
                .map(|res| {
                    let mut len = res.versions.first().map(|v| v.version.len()).unwrap_or(0);
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
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
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
                    let mut version = res.versions.first().map(|v| v.version.as_str()).unwrap_or("Unknown").to_string();
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

            let mut scrollbar_state = ScrollbarState::new(results.len())
                .position(list_state.selected().unwrap_or(0));

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

            // Render scrollbar
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"));

            let mut scrollbar_state = ScrollbarState::new(results.len())
                .position(list_state.selected().unwrap_or(0));

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

fn render_version_selection(
    frame: &mut Frame,
    area: Rect,
    is_fetching: bool,
    error: Option<&str>,
    versions: &[VersionInfo],
    list_state: &mut ListState,
    inputs: &[Input],
    current_versions: &[ChannelVersion],
    mode: Mode,
) {
    let list_title = if is_fetching {
        " Searching for versions... (󱑆) "
    } else {
        " Select Input or Version (Enter to add) "
    };

    let mut all_items = Vec::new();

    // 1. Add Available Inputs
    if mode == Mode::Flake {
        for input in inputs {
            let channel_name = crate::state::domain::extract_channel(input);
            if let Some(cv) = current_versions.iter().find(|v| v.channel == channel_name) {
                let version = cv.locked_version.as_ref().unwrap_or(&cv.version).clone();
                let rev = input.rev.as_deref().unwrap_or("-------");
                
                all_items.push((
                    version.clone(),
                    rev.to_string(),
                    false, // Inputs don't have unfree marker here for simplicity, or we could fetch it
                    Some((input.name.clone(), channel_name)),
                    false, // is_system
                ));
            }
        }
    }

    // 2. Add Historical Versions
    for v in versions {
        all_items.push((
            v.version.clone(),
            v.hash.clone(),
            v.is_unfree,
            None,
            v.is_system, // is_system
        ));
    }

    // 4. Sort by version (descending)
    all_items.sort_by(|a, b| b.0.cmp(&a.0));

    if all_items.is_empty() && !is_fetching {
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
                Line::from(vec![Span::styled("Error fetching versions:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))]),
                Line::from(vec![Span::styled(err, Style::default().fg(Color::Red))]),
                Line::from(""),
                Line::from(vec![Span::styled("Please check your internet connection.", Style::default().fg(Color::DarkGray))]),
            ];
            let error_para = Paragraph::new(error_text).alignment(Alignment::Center);
            frame.render_widget(error_para, vertical_chunks[1]);
        } else {
            let empty = Paragraph::new("No versions found.").alignment(Alignment::Center);
            frame.render_widget(empty, vertical_chunks[1]);
        }
    } else if is_fetching && all_items.is_empty() {
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
                format!("{:30}", "Version (Input)"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                "Hash",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let header = Paragraph::new(Line::from(header_spans));

        let items: Vec<ListItem> = all_items
            .into_iter()
            .map(|(version, hash, is_unfree, input_info, is_system)| {
                let unfree_marker = if is_unfree {
                    Span::styled("$ ", Style::default().fg(Color::Green))
                } else {
                    Span::raw("  ")
                };

                let mut display_name = version.clone();
                let mut style = Style::default().add_modifier(Modifier::BOLD);

                if let Some((name, channel)) = input_info {
                    let color = get_channel_color(&channel);
                    display_name = format!("{} ({})", version, name);
                    style = style.fg(color);
                } else if is_system {
                    display_name = format!("{} [System]", version);
                    style = style.fg(Color::Yellow);
                }

                let spans = vec![
                    unfree_marker,
                    Span::styled(format!("{:30}", display_name), style),
                    Span::raw(" | "),
                    Span::styled(hash, Style::default().fg(Color::Cyan)),
                ];
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        frame.render_widget(header, list_chunks[0]);

        let list_len = items.len();
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

        let mut scrollbar_state = ScrollbarState::new(list_len)
            .position(list_state.selected().unwrap_or(0));

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
