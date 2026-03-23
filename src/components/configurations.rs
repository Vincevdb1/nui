use ratatui::{prelude::*, widgets::*};
use crate::nix::Configuration;

pub fn render(configurations: &[Configuration], frame: &mut Frame, area: Rect, is_selected: bool) {
    let list_items: Vec<ListItem> = configurations
        .iter()
        .map(|config| {
            ListItem::new(Line::from(vec![Span::styled(
                format!("• {}", config.path),
                Style::default().add_modifier(Modifier::BOLD),
            )]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" [4] Configurations ({}) ", configurations.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let list = List::new(list_items);
    frame.render_widget(list, inner_area);
}
