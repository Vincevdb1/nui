use ratatui::{prelude::*, widgets::*};
use crate::nix::extract_inputs;

pub fn render(frame: &mut Frame, area: Rect, is_selected: bool) {
    let file = "flake.nix";

    let content = match std::fs::read_to_string(file) {
        Ok(content) => content,
        Err(_) => String::new(),
    };

    let inputs = extract_inputs(&content);
    let list_items: Vec<ListItem> = inputs
        .into_iter()
        .map(|input| ListItem::new(format!("• {}", input)))
        .collect();

    let block = Block::default()
        .title(" [3] Inputs ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected { Style::default().fg(Color::Yellow) } else { Style::default() });

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let l = List::new(list_items);
    frame.render_widget(l, inner_area);
}
