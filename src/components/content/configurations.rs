use ratatui::{prelude::*, widgets::*};

pub fn render(frame: &mut Frame, area: Rect) {
    let p = Paragraph::new("This is the CONTENT for the Configurations [4]")
        .alignment(Alignment::Center);
    frame.render_widget(p, area);
}
