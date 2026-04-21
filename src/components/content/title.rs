
use ratatui::{prelude::*, widgets::*};

pub fn render(frame: &mut Frame, area: Rect) {
    let version = env!("CARGO_PKG_VERSION");
    let mut text = Text::from(vec![
        Line::from(r"             _ ").cyan().bold(),
        Line::from(r" _ __  _   _(_)").cyan().bold(),
        Line::from(r"| '_ \| | | | |").cyan().bold(),
        Line::from(r"| | | | |_| | |").cyan().bold(),
        Line::from(vec![
            Span::styled(
                r"|_| |_|\__,_|_|",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" v{}", version), Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    text.lines.extend(vec![
        Line::from(""),
        Line::from("NUI - Nix User Interface").bold().cyan(),
        Line::from("A terminal interface for managing Nix flakes.").gray(),
        Line::from(""),
        Line::from("Thank you for using NUI").bold(),
        Line::from(""),
        Line::from(vec!["Github: ".into(), "https://github.com/Vincevdb1/nui".underlined()]).fg(Color::DarkGray),
        Line::from(" Vincent Vandebosch").italic().fg(Color::DarkGray),
        Line::from("󰿃 MIT License").italic().fg(Color::DarkGray),
    ]);

    let p = Paragraph::new(text).wrap(Wrap { trim: false });

    frame.render_widget(p, area);
}
