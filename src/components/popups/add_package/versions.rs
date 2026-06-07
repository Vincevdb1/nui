use crate::nix::Input;
use crate::state::Mode;
use crate::state::domain::{ChannelVersion, VersionInfo};
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

pub fn render_version_selection(
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
    all_items.sort_by(|a, b| compare_versions(&b.0, &a.0));

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
                Line::from(vec![Span::styled(
                    "Error fetching versions:",
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
            Span::styled("Hash", Style::default().add_modifier(Modifier::BOLD)),
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

        let mut scrollbar_state =
            ScrollbarState::new(list_len).position(list_state.selected().unwrap_or(0));

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

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split(|c: char| !c.is_alphanumeric()).collect();
    let b_parts: Vec<&str> = b.split(|c: char| !c.is_alphanumeric()).collect();

    for (a_p, b_p) in a_parts.iter().zip(b_parts.iter()) {
        let a_num = a_p.parse::<u64>();
        let b_num = b_p.parse::<u64>();

        match (a_num, b_num) {
            (Ok(an), Ok(bn)) => {
                if an != bn {
                    return an.cmp(&bn);
                }
            }
            _ => {
                if a_p != b_p {
                    return a_p.cmp(b_p);
                }
            }
        }
    }

    a_parts.len().cmp(&b_parts.len())
}
