use crate::log::LogEntry;
use crate::ui::widgets::{Selectable, panel};
use ratatui::{prelude::*, widgets::*};

#[derive(Default)]
pub struct LogState {
    pub entries: Vec<LogEntry>,
    pub list: ListState,
}

impl LogState {
    fn line_count(&self) -> usize {
        self.entries.iter().map(LogEntry::height).sum()
    }

    /// Appends an entry and follows the tail.
    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push(entry);
        self.list.select(self.line_count().checked_sub(1));
    }

    pub fn scroll(&mut self, forward: bool) {
        if !self.entries.is_empty() {
            let len = self.line_count();
            self.list.step(len, forward);
        }
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    log: &mut LogState,
    focused: bool,
    progress: Option<&str>,
) {
    let title = match progress {
        Some(p) => format!(" [5] Command Log (Updating NXV Index: {}) ", p),
        None => " [5] Command Log ".to_string(),
    };

    let entry_lines = |entry: &LogEntry| -> Vec<ListItem<'static>> {
        let (header, color, body): (String, Color, Vec<String>) = match entry {
            LogEntry::Action { action, command } => {
                (action.clone(), Color::Cyan, vec![command.clone()])
            }
            LogEntry::Output { header, message } => (
                header.clone(),
                Color::Magenta,
                message.lines().map(str::to_string).collect(),
            ),
            LogEntry::Info(msg) => ("Info".to_string(), Color::Blue, vec![msg.clone()]),
        };
        std::iter::once(Line::styled(
            header,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .chain(body.into_iter().map(|l| Line::raw(format!("  {}", l))))
        .chain(std::iter::once(Line::raw("")))
        .map(ListItem::new)
        .collect()
    };

    let items: Vec<ListItem> = log.entries.iter().flat_map(entry_lines).collect();
    frame.render_stateful_widget(
        List::new(items).block(panel(title, focused)),
        area,
        &mut log.list,
    );
}
