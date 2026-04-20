use crate::app::App;
use ratatui::{prelude::*, widgets::*};

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = app
        .shell_packages
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    " • ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    p,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]))
        })
        .collect();

    let is_empty = items.is_empty();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    if is_empty {
        let p = Paragraph::new(vec![
            Line::from("No packages added yet."),
            Line::from(""),
            Line::from("Press 'a' to add packages to your temporary shell."),
        ])
        .alignment(Alignment::Center);

        // Center the paragraph vertically
        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Min(3),
                Constraint::Percentage(45),
            ])
            .split(area)[1];

        frame.render_widget(p, area);
    } else {
        frame.render_stateful_widget(list, area, &mut app.ui.shell_package_list_state);
    }
}
