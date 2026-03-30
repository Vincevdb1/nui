use crate::app::App;
use ratatui::{prelude::*, widgets::*};

pub mod inputs;
pub mod packages;
pub mod title;

pub fn render(app: &App, frame: &mut Frame, area: Rect, selected_index: usize) {
    let title = match selected_index {
        2 | 4 => {
            let context_name = app
                .nix_files
                .get(app.selected_nix_file_index)
                .map(|f| f.name.as_str())
                .unwrap_or("None");
            let config_path = app
                .configurations
                .get(app.selected_configuration_index)
                .map(|c| c.path.as_str())
                .unwrap_or("None");
            format!(" [0] Content: {} / {} ", context_name, config_path)
        }
        3 => " [0] Content: Inputs ".to_string(),
        1 => " [0] Content: Title ".to_string(),
        _ => " [0] Content ".to_string(),
    };

    let is_selected = selected_index == 0;
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        });

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    match selected_index {
        1 => title::render(frame, inner_area),
        2 | 4 => packages::render(app, frame, inner_area),
        3 => inputs::render(&app.inputs, frame, inner_area),
        _ => {
            let p = Paragraph::new("Select a box in the first column to view content.")
                .alignment(Alignment::Center);
            frame.render_widget(p, inner_area);
        }
    }
}
