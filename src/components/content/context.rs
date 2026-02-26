use ratatui::{prelude::*, widgets::*};

pub fn render(frame: &mut Frame, area: Rect) {
    let file = "flake.nix";

    let content = match std::fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("error reading file: {}", err);
            return;
        }
    };
    let parse = rnix::Root::parse(&content);

    let p = Paragraph::new(format!("{:#?}", parse.tree()))
        .alignment(Alignment::Center);
    frame.render_widget(p, area);
}
