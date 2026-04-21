use crate::app::App;
use ratatui::{prelude::*, widgets::*};

pub mod inputs;
pub mod packages;
pub mod shell;
pub mod title;

pub fn render(app: &mut App, frame: &mut Frame, area: Rect, selected_index: usize) {
    let title = match selected_index {
        2 | 4 => {
            let context_name = app
                .domain
                .nix_files
                .get(app.ui.selected_nix_file_index)
                .map(|f| f.name.as_str())
                .unwrap_or("None");
            let config_path = app
                .domain
                .configurations
                .get(app.ui.selected_configuration_index)
                .map(|c| c.path.as_str())
                .unwrap_or("None");
            format!(" Content: {} / {} ", context_name, config_path)
        }
        3 => " Content: Inputs ".to_string(),
        1 => {
            if app.mode == crate::state::Mode::Shell {
                " Shell: Packages ".to_string()
            } else {
                " Content: Title ".to_string()
            }
        }
        _ => " Content ".to_string(),
    };

    let is_selected = app.mode == crate::state::Mode::Shell && selected_index == 1;
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
        1 => {
            if app.mode == crate::state::Mode::Shell {
                shell::render(app, frame, inner_area);
            } else {
                title::render(
                    frame,
                    inner_area,
                    app.ui.selected_index == 1,
                    app.domain.nh_version.clone(),
                    app.domain.nxv_version.clone(),
                );
            }
        }
        2 | 4 => packages::render(app, frame, inner_area),
        3 => inputs::render(&app.domain.inputs, frame, inner_area),
        _ => {
            let p = Paragraph::new("Select a box in the first column to view content.")
                .alignment(Alignment::Center);
            frame.render_widget(p, inner_area);
        }
    }
}
