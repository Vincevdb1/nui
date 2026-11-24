use crate::app::{App, Effect, Popup};
use crate::flake::edit::Snapshot;
use crate::ui::widgets::popup_frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

/// An edit whose devShell check failed. nui does not revert by itself, because the check
/// also fails for reasons the edit did not cause, such as no network.
pub struct CheckFailure {
    pub error: String,
    pub snapshot: Snapshot,
}

pub fn on_key(app: &mut App, failure: CheckFailure, key: KeyEvent) -> Popup {
    match key.code {
        KeyCode::Char('r') => {
            app.effects.push(Effect::Restore(failure.snapshot));
            Popup::None
        }
        KeyCode::Char('k') | KeyCode::Esc => {
            app.flake.last_edit = Some(failure.snapshot);
            Popup::None
        }
        _ => Popup::CheckFailed(failure),
    }
}

pub fn render(frame: &mut Frame, failure: &CheckFailure) {
    let area = popup_frame(frame, 80, 60, "Edit failed the check", Color::Red);
    let [text, hint] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
        .areas(area.inner(Margin::new(2, 1)));
    frame.render_widget(
        Paragraph::new(failure.error.as_str()).wrap(Wrap { trim: false }),
        text,
    );
    frame.render_widget(
        Paragraph::new("r: Revert the edit | k/Esc: Keep it")
            .style(Style::default().fg(Color::Gray)),
        hint,
    );
}
