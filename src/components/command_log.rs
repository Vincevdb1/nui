use ratatui::{prelude::*, widgets::*};
use std::sync::OnceLock;
use std::sync::mpsc::Sender;

static LOG_SENDER: OnceLock<Sender<String>> = OnceLock::new();

pub fn init_logger(tx: Sender<String>) {
    let _ = LOG_SENDER.set(tx);
}

pub fn command_log(msg: impl Into<String>) {
    if let Some(tx) = LOG_SENDER.get() {
        let _ = tx.send(msg.into());
    }
}

pub fn render(frame: &mut Frame, area: Rect, is_selected: bool, logs: &[String], state: &mut ListState) {
    let block = Block::default()
        .title(" [5] Command Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let list_items: Vec<ListItem> = logs
        .iter()
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let list = List::new(list_items)
        .block(block)
        .highlight_style(Style::default()) // Don't highlight differently, we just use selection for scrolling
        ;
    
    frame.render_stateful_widget(list, area, state);
}
