use ratatui::{prelude::*, widgets::*};

pub trait Selectable {
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, index: Option<usize>);

    /// Moves the selection with wrap-around; an empty list clears it.
    fn step(&mut self, len: usize, forward: bool) {
        self.select(wrap(self.selected(), len, forward));
    }
}

impl Selectable for ListState {
    fn selected(&self) -> Option<usize> {
        ListState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        ListState::select(self, index)
    }
}

impl Selectable for TableState {
    fn selected(&self) -> Option<usize> {
        TableState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        TableState::select(self, index)
    }
}

pub fn wrap(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match current {
        None => 0,
        Some(i) if forward => {
            if i + 1 >= len {
                0
            } else {
                i + 1
            }
        }
        Some(0) => len - 1,
        Some(i) => (i - 1).min(len - 1),
    })
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

/// Clears a centered area and draws a titled popup frame in it.
pub fn popup_frame(
    frame: &mut Frame,
    percent_x: u16,
    percent_y: u16,
    title: &str,
    color: Color,
) -> Rect {
    let area = centered_rect(percent_x, percent_y, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .title(format!(" [ {} ] ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color)),
        area,
    );
    area
}

pub fn panel(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        })
}

/// A table header with a rule underneath; `│` columns become `┼` in the rule.
pub fn ruled_header<'a>(titles: &[&'a str]) -> Row<'a> {
    let cells = titles.iter().map(|&title| {
        let rule = match title {
            "│" => "┼".to_string(),
            "" => String::new(),
            _ => "─".repeat(100),
        };
        Cell::from(Text::from(vec![
            Line::styled(title, Style::default().add_modifier(Modifier::BOLD)),
            Line::raw(rule),
        ]))
    });
    Row::new(cells).height(2)
}

pub fn highlight() -> Style {
    Style::default()
        .bg(Color::Cyan)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

pub fn scrollbar(frame: &mut Frame, area: Rect, len: usize, position: Option<usize>) {
    let mut state = ScrollbarState::new(len).position(position.unwrap_or(0));
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// A message centered in `area`, or an error explanation when `error` is set.
pub fn placeholder(frame: &mut Frame, area: Rect, message: &str, error: Option<(&str, &str)>) {
    let lines = match error {
        Some((what, err)) => vec![
            Line::styled(
                format!("Error {}:", what),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::styled(err.to_string(), Style::default().fg(Color::Red)),
            Line::raw(""),
            Line::styled(
                "Please check your internet connection.",
                Style::default().fg(Color::DarkGray),
            ),
        ],
        None => vec![Line::raw(message.to_string())],
    };
    let rows = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Length(lines.len() as u16),
        Constraint::Percentage(50),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rows[1]);
}

pub fn channel_color(channel: &str) -> Color {
    const COLORS: [Color; 12] = [
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
    if channel == "system" {
        return Color::Cyan;
    }
    let hash = channel
        .chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
    COLORS[hash as usize % COLORS.len()]
}

pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        format!("{}...", text.chars().take(max - 3).collect::<String>())
    } else {
        text.to_string()
    }
}

pub fn unfree_marker(is_unfree: bool) -> Span<'static> {
    if is_unfree {
        Span::styled("$ ", Style::default().fg(Color::Green))
    } else {
        Span::raw("  ")
    }
}

pub fn mark_marker(any_marked: bool, marked: bool) -> Span<'static> {
    match (any_marked, marked) {
        (false, _) => Span::raw(""),
        (true, true) => Span::styled(" ● ", Style::default().fg(Color::Yellow)),
        (true, false) => Span::raw(" ○ "),
    }
}

pub fn text_field<'a>(value: &'a str, title: &'a str, focused: bool) -> Paragraph<'a> {
    Paragraph::new(value).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }),
    )
}

pub fn footer(text: &str) -> Paragraph<'_> {
    Paragraph::new(text).style(Style::default().fg(Color::DarkGray))
}
