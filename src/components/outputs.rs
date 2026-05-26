use crate::nix::Output;
use ratatui::{prelude::*, widgets::*};

pub struct OutputsProps<'a> {
    pub outputs: &'a [Output],
    pub is_selected: bool,
    pub selected_index: usize,
}

pub fn render(
    props: &OutputsProps,
    frame: &mut Frame,
    area: Rect,
) {
    let list_items: Vec<ListItem> = props.outputs
        .iter()
        .enumerate()
        .map(|(i, output)| {
            let (style, prefix_style) = if i == props.selected_index {
                (
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
                )
            } else {
                (Style::default(), Style::default().fg(Color::DarkGray))
            };

            let type_prefix = match output.config_type.as_str() {
                "devShells" => "SHELL",
                "nixosConfigurations" => "OS",
                "homeConfigurations" => "HM",
                "darwinConfigurations" => "DRWN",
                _ => "???",
            };

            let display_path = if output.config_type == "devShells" {
                if let Some((system, name)) = output.path.split_once('.') {
                    format!("{} ({})", name, system)
                } else {
                    output.path.clone()
                }
            } else {
                output.path.clone()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("[{}] ", type_prefix), prefix_style),
                Span::styled(display_path, style),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" [4] Outputs ({}) ", props.outputs.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if props.is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let list = List::new(list_items);
    frame.render_widget(list, inner_area);
}
