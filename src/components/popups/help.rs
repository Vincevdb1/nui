use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

use crate::components::popups::centered_rect;

pub fn render(frame: &mut Frame) {
    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" [ Help & Legend ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Legend
            Constraint::Min(0),    // Keybinds
            Constraint::Length(1), // Footer
        ])
        .split(area);

    // Legend
    let legend_text = vec![
        Span::styled(" ● ", Style::default().fg(Color::Yellow)),
        Span::raw("Selected  "),
        Span::styled(" $ ", Style::default().fg(Color::Green)),
        Span::raw("Unfree  "),
        Span::styled(" 󰚰 ", Style::default().fg(Color::Yellow)),
        Span::raw("Update  "),
        Span::styled(" 󰐃 ", Style::default().fg(Color::Cyan)),
        Span::raw("Pinned"),
    ];
    let legend = Paragraph::new(Line::from(legend_text))
        .block(Block::default().title(" Legend ").borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(legend, chunks[0]);

    // Keybinds
    let keybinds = vec![
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
        ("p", "Pin/Unpin package"),
        ("s", "Search/Filter (in Add Package)"),
        ("a", "Add Package/Input"),
        ("d", "Remove Package/Input"),
        ("i", "Show Package Details"),
        ("Space", "Multi-select packages"),
    ];

    let rows: Vec<Row> = keybinds
        .into_iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(Span::styled(
                    key,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(desc)),
            ])
        })
        .collect();

    let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(20)])
        .block(Block::default().title(" Keybindings ").borders(Borders::ALL))
        .column_spacing(2);
    frame.render_widget(table, chunks[1]);

    let footer = Paragraph::new("Press '?' or 'Esc' to close")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}
