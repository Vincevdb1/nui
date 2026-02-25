use ratatui::{prelude::*, widgets::*};
pub fn render(frame: &mut Frame, area: Rect, is_selected: bool) {
    let block = Block::default()
        .title(" [5] Command Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected { Style::default().fg(Color::Yellow) } else { Style::default() });
    frame.render_widget(block, area);
}
