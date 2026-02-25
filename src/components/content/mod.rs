use ratatui::{prelude::*, widgets::*};

pub mod title;
pub mod context;
pub mod inputs;
pub mod configurations;

pub fn render(frame: &mut Frame, area: Rect, selected_index: usize) {
    let is_selected = selected_index == 0;
    let block = Block::default()
        .title(" [0] Content ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected { 
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) 
        } else { 
            Style::default() 
        });
    
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    match selected_index {
        1 => title::render(frame, inner_area),
        2 => context::render(frame, inner_area),
        3 => inputs::render(frame, inner_area),
        4 => configurations::render(frame, inner_area),
        _ => {
            let p = Paragraph::new("Select a box in the first column to view content.")
                .alignment(Alignment::Center);
            frame.render_widget(p, inner_area);
        }
    }
}
