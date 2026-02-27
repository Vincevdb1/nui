use ratatui::{prelude::*, widgets::*};
use crate::app::App;

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let block = Block::default()
        .title(" [2] Context ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected { Style::default().fg(Color::Yellow) } else { Style::default() });

    let items: Vec<ListItem> = app.nix_files.iter().enumerate().map(|(i, nix_file)| {
        let style = if i == app.selected_nix_file_index {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(format!("• {}", nix_file.name)).style(style)
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_symbol(">> ");

    frame.render_widget(list, area);
}
