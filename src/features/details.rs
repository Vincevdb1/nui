use crate::app::Popup;
use crate::backend::search::{ChannelVersion, SearchResult};
use crate::ui::widgets::{channel_color, popup_frame};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

pub fn on_key(result: SearchResult, key: KeyEvent) -> Popup {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => Popup::None,
        _ => Popup::Details(result),
    }
}

/// Details for an installed package, which has a single known version.
pub fn installed(
    name: &str,
    description: &str,
    version: &str,
    channel: &str,
    is_unfree: bool,
    source_input: Option<String>,
    hash: Option<String>,
) -> SearchResult {
    SearchResult {
        name: name.to_string(),
        description: description.to_string(),
        versions: vec![ChannelVersion {
            version: version.to_string(),
            channel: channel.to_string(),
            locked_version: None,
        }],
        platforms: Vec::new(),
        is_unfree,
        source_input,
        hash,
    }
}

pub fn render(frame: &mut Frame, result: &SearchResult) {
    let area = popup_frame(
        frame,
        60,
        50,
        &format!("{} Details", result.name),
        Color::Magenta,
    );
    let inner = Block::default().borders(Borders::ALL).inner(area);

    let platforms = result.platforms.join(", ");
    let platforms_height = if result.platforms.is_empty() {
        0
    } else {
        let width = inner.width.saturating_sub(2) as usize;
        if width == 0 {
            1
        } else {
            ("Platforms: ".len() + platforms.len()).div_ceil(width) as u16
        }
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(u16::from(result.source_input.is_some())),
        Constraint::Length(result.versions.len() as u16 + 1),
        Constraint::Length(platforms_height),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(inner);

    let label = |text: &'static str| Span::styled(text, Style::default().fg(Color::DarkGray));

    let mut name = vec![
        label("Attribute: "),
        Span::styled(&result.name, Style::default().add_modifier(Modifier::BOLD)),
    ];
    if result.is_unfree {
        name.push(Span::styled(" [UNFREE]", Style::default().fg(Color::Green)));
    }
    frame.render_widget(Paragraph::new(Line::from(name)), chunks[0]);

    if let Some(source) = &result.source_input {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                label("Source: "),
                Span::styled(source, Style::default().fg(Color::Yellow)),
            ])),
            chunks[1],
        );
    }

    let versions: Vec<Line> = std::iter::once(Line::from(label("Available Versions:")))
        .chain(result.versions.iter().map(|v| {
            Line::from(vec![
                Span::raw("  • "),
                Span::styled(&v.channel, Style::default().fg(channel_color(&v.channel))),
                Span::raw(": "),
                Span::styled(&v.version, Style::default().add_modifier(Modifier::BOLD)),
            ])
        }))
        .collect();
    frame.render_widget(Paragraph::new(versions), chunks[2]);

    if !result.platforms.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![label("Platforms: "), Span::raw(platforms)]))
                .wrap(Wrap { trim: true }),
            chunks[3],
        );
    }

    let description = if result.description.is_empty() {
        "No description available."
    } else {
        &result.description
    };
    frame.render_widget(
        Paragraph::new(description)
            .block(
                Block::default()
                    .title(" Description ")
                    .borders(Borders::TOP),
            )
            .wrap(Wrap { trim: true }),
        chunks[4],
    );

    frame.render_widget(
        Paragraph::new("Press Tab, q, or Esc to close")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}
