use crate::state::domain::SearchResult;
use ratatui::{
    Frame,
    layout::{Alignment, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::components::popups::centered_rect;

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
            Constraint::Length(1),                                                 // Name
            Constraint::Length(if result.source_input.is_some() { 1 } else { 0 }), // Source
            Constraint::Length(result.versions.len() as u16 + 1),                  // Versions
            Constraint::Length(platforms_height),                                  // Platforms
            Constraint::Min(0),                                                    // Description
            Constraint::Length(1),                                                 // Footer
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

use ratatui::layout::Constraint;
