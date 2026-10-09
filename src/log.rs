use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub enum LogEntry {
    Action { action: String, command: String },
    Output { header: String, message: String },
    Info(String),
}

impl LogEntry {
    /// Rows the entry occupies in the log panel.
    pub fn height(&self) -> usize {
        match self {
            Self::Output { message, .. } => 2 + message.lines().count(),
            Self::Action { .. } | Self::Info(_) => 3,
        }
    }
}

type Sink = Box<dyn Fn(LogEntry) + Send + Sync>;

// Global so background threads can log without threading a sender through every call.
static SINK: OnceLock<Sink> = OnceLock::new();

pub fn init(sink: impl Fn(LogEntry) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(sink));
}

fn emit(entry: LogEntry) {
    if let Some(sink) = SINK.get() {
        sink(entry);
    }
}

pub fn action(action: impl Into<String>, command: impl Into<String>) {
    emit(LogEntry::Action {
        action: action.into(),
        command: command.into(),
    });
}

pub fn output(header: impl Into<String>, message: impl Into<String>) {
    emit(LogEntry::Output {
        header: header.into(),
        message: message.into(),
    });
}

pub fn info(message: impl Into<String>) {
    emit(LogEntry::Info(message.into()));
}
