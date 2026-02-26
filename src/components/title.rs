use ratatui::{prelude::*, widgets::*};
pub fn render(frame: &mut Frame, area: Rect, is_selected: bool) {
    let block = Block::default()
        .title(" [1] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected { Style::default().fg(Color::Yellow) } else { Style::default() });
    let p = Paragraph::new(" NUI ")
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Left)
        .block(block);
    frame.render_widget(p, area);
}
