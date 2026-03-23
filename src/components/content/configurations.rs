use ratatui::{prelude::*, widgets::*};
use crate::nix::Configuration;

pub fn render(_configurations: &[Configuration], frame: &mut Frame, area: Rect) {
    let p = Paragraph::new("Select a configuration from the sidebar to view details.")
        .alignment(Alignment::Center);
    frame.render_widget(p, area);
}
