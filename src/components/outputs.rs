use crate::nix::Output;
use ratatui::{prelude::*, widgets::*};

pub fn render(
    outputs: &[Output],
    frame: &mut Frame,
    area: Rect,
    is_selected: bool,
    selected_index: usize,
) {
    let list_items: Vec<ListItem> = outputs
        .iter()
        .enumerate()
        .map(|(i, output)| {
            let style = if i == selected_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!("• {}", output.path),
                style,
            )]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" [4] Outputs ({}) ", outputs.len()))
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
