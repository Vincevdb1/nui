use ratatui::{prelude::*, widgets::*};

pub struct ContextProps<'a> {
    pub nix_files: &'a [crate::context::NixFile],
    pub selected_nix_file_index: usize,
    pub is_selected: bool,
}

pub fn render(props: &ContextProps, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" [2] Context ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if props.is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let items: Vec<ListItem> = props
        .nix_files
        .iter()
        .enumerate()
        .map(|(i, nix_file)| {
            let style = if i == props.selected_nix_file_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("• {}", nix_file.name)).style(style)
        })
        .collect();

    let list = List::new(items).block(block).highlight_symbol(">> ");

    frame.render_widget(list, area);
}
