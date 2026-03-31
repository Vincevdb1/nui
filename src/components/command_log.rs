use ratatui::{prelude::*, widgets::*};
use std::sync::OnceLock;
use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum LogEntry {
    Action {
        action: String,
        command: String,
    },
    Output {
        header: String,
        message: String,
    },
    Info(String),
}

static LOG_SENDER: OnceLock<Sender<LogEntry>> = OnceLock::new();

pub fn init_logger(tx: Sender<LogEntry>) {
    let _ = LOG_SENDER.set(tx);
}

pub fn command_log(msg: impl Into<LogEntry>) {
    if let Some(tx) = LOG_SENDER.get() {
        let _ = tx.send(msg.into());
    }
}

impl From<String> for LogEntry {
    fn from(s: String) -> Self {
        LogEntry::Info(s)
    }
}

impl From<&str> for LogEntry {
    fn from(s: &str) -> Self {
        LogEntry::Info(s.to_string())
    }
}

pub fn log_action(action: impl Into<String>, command: impl Into<String>) {
    command_log(LogEntry::Action {
        action: action.into(),
        command: command.into(),
    });
}

pub fn log_output(header: impl Into<String>, message: impl Into<String>) {
    command_log(LogEntry::Output {
        header: header.into(),
        message: message.into(),
    });
}

pub fn count_lines(logs: &[LogEntry]) -> usize {
    let mut count = 0;
    for log in logs {
        match log {
            LogEntry::Action { .. } => count += 3,
            LogEntry::Output { message, .. } => {
                count += 2 + message.lines().count();
            }
            LogEntry::Info(_) => count += 3,
        }
    }
    count
}

pub fn render(frame: &mut Frame, area: Rect, is_selected: bool, logs: &[LogEntry], state: &mut ListState) {
    let block = Block::default()
        .title(" [5] Command Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let mut list_items: Vec<ListItem> = Vec::new();

    for log in logs {
        match log {
            LogEntry::Action { action, command } => {
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled(action.as_str(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ])));
                list_items.push(ListItem::new(Line::from(vec![
                    Span::raw(format!("  {}", command)),
                ])));
                list_items.push(ListItem::new(Line::from("")));
            }
            LogEntry::Output { header, message } => {
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled(header.as_str(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                ])));
                for line in message.lines() {
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::raw(format!("  {}", line)),
                    ])));
                }
                list_items.push(ListItem::new(Line::from("")));
            }
            LogEntry::Info(msg) => {
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled("Info", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                ])));
                list_items.push(ListItem::new(Line::from(vec![
                    Span::raw(format!("  {}", msg)),
                ])));
                list_items.push(ListItem::new(Line::from("")));
            }
        }
    }

    let list = List::new(list_items)
        .block(block)
        .highlight_style(Style::default())
        ;
    
    frame.render_stateful_widget(list, area, state);
}
