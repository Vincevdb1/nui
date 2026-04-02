use crate::app::SearchResult;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(
    frame: &mut Frame,
    query: &str,
    results: &[SearchResult],
    channels: &[String],
    is_searching: bool,
    list_state: &mut ListState,
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
            Constraint::Length(1), // Channel legend
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

    // Channel Legend
    let mut legend_spans = Vec::new();
    for (i, channel) in channels.iter().enumerate() {
        let color = get_channel_color(channel);
        legend_spans.push(Span::styled(channel, Style::default().fg(color)));
        if i < channels.len() - 1 {
            legend_spans.push(Span::raw(" | "));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(legend_spans)), chunks[1]);

    let list_title = if is_searching {
        " Searching... "
    } else {
        " Search Results (Enter to add) "
    };

    if results.is_empty() {
        let block = Block::default()
            .title(list_title)
            .borders(Borders::ALL);
        frame.render_widget(block, chunks[2]);

        let area = chunks[2];
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
        let max_name_width = results.iter().map(|res| res.name.len()).max().unwrap_or(0).max(10);
        
        let mut max_version_widths = Vec::new();
        for channel in channels {
            let max_w = results.iter()
                .map(|res| {
                    res.versions.iter()
                        .find(|v| v.channel == *channel)
                        .map(|v| v.version.len())
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            max_version_widths.push(max_w.max(5)); // Minimum width for a version column
        }

        let items: Vec<ListItem> = results
            .iter()
            .map(|res| {
                let mut spans = vec![
                    Span::styled(
                        format!("  {:width$}", res.name, width = max_name_width),
                        Style::default().add_modifier(Modifier::BOLD)
                    ),
                ];

                for (i, channel) in channels.iter().enumerate() {
                    spans.push(Span::raw(" | "));
                    let version_opt = res.versions.iter().find(|v| v.channel == *channel);
                    let width = max_version_widths[i];
                    
                    if let Some(v) = version_opt {
                        let color = get_channel_color(channel);
                        spans.push(Span::styled(
                            format!("{:width$}", v.version, width = width),
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
                    spans.push(Span::styled(desc_trimmed, Style::default().fg(Color::DarkGray)));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(list_title)
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[2], list_state);
    }

    let footer_text = "Type: Search | Enter: Add | Tab: Details | Esc/q: Close";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}

pub fn render_details(
    frame: &mut Frame,
    result: &SearchResult,
) {
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
        if width == 0 { 1 } else { ((text_len + width - 1) / width) as u16 }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // Name
            Constraint::Length(result.versions.len() as u16 + 1), // Versions
            Constraint::Length(platforms_height), // Platforms
            Constraint::Min(0), // Description
            Constraint::Length(1), // Footer
        ])
        .split(inner_area);

    // Name
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Attribute: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&result.name, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        chunks[0]
    );

    // Versions
    let mut version_lines = vec![Line::from(Span::styled("Available Versions:", Style::default().fg(Color::DarkGray)))];
    for ver in &result.versions {
        let color = get_channel_color(&ver.channel);
        version_lines.push(Line::from(vec![
            Span::raw("  • "),
            Span::styled(&ver.channel, Style::default().fg(color)),
            Span::raw(": "),
            Span::styled(&ver.version, Style::default().add_modifier(Modifier::BOLD)),
        ]));
    }
    frame.render_widget(Paragraph::new(version_lines), chunks[1]);

    // Platforms
    if !result.platforms.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Platforms: ", Style::default().fg(Color::DarkGray)),
                Span::raw(platforms_text),
            ]))
            .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[2]
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
            .block(Block::default().title(" Description ").borders(Borders::TOP))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        chunks[3]
    );

    // Footer
    frame.render_widget(
        Paragraph::new("Press Tab, q, or Esc to close")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[4]
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
