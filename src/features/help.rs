use crate::app::Popup;
use crate::ui::widgets::popup_frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

pub fn on_key(key: KeyEvent) -> Popup {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Popup::None,
        _ => Popup::Help,
    }
}

const KEYBINDS: [(&str, &str); 17] = [
    ("?", "Show this help"),
    ("q", "Quit application"),
    ("m", "Switch Mode (Flake/Shell)"),
    ("t", "Browse Templates"),
    ("T", "Save Shell as Template (Shell Mode)"),
    ("Tab", "Next focus / Switch pane"),
    ("1-5", "Select specific tab"),
    ("j/k", "Navigate list / Scroll"),
    ("Enter", "Select / Toggle / Add"),
    ("Esc", "Close popup / Back"),
    ("v", "Change package version (Shell Mode)"),
    ("p", "Pin/Unpin package (Flake Mode)"),
    ("s", "Start shell (Shell Mode)"),
    ("a", "Add Package/Input"),
    ("d", "Remove Package/Input"),
    ("i", "Show Package Details"),
    ("Space", "Multi-select packages"),
];

pub fn render(frame: &mut Frame) {
    let area = popup_frame(frame, 60, 60, "Help & Legend", Color::Yellow);
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .margin(2)
    .split(area);

    let legend = Line::from(vec![
        Span::styled(" ● ", Style::default().fg(Color::Yellow)),
        Span::raw("Selected  "),
        Span::styled(" $ ", Style::default().fg(Color::Green)),
        Span::raw("Unfree  "),
        Span::styled(" 󰚰 ", Style::default().fg(Color::Yellow)),
        Span::raw("Update  "),
        Span::styled(" 󰐃 ", Style::default().fg(Color::Cyan)),
        Span::raw("Pinned"),
    ]);
    frame.render_widget(
        Paragraph::new(legend)
            .block(Block::default().title(" Legend ").borders(Borders::ALL))
            .alignment(Alignment::Center),
        chunks[0],
    );

    let rows = KEYBINDS.iter().map(|(key, desc)| {
        Row::new(vec![
            Cell::from(Span::styled(
                *key,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Cell::from(*desc),
        ])
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Length(10), Constraint::Min(20)])
            .block(
                Block::default()
                    .title(" Keybindings ")
                    .borders(Borders::ALL),
            )
            .column_spacing(2),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new("Press '?' or 'Esc' to close")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
    );
}
