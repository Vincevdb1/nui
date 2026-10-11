use nui::features::log::LogState;
use nui::log::{self, LogEntry};
use ratatui::widgets::ListState;
use std::sync::{Arc, Mutex};

fn output(message: &str) -> LogEntry {
    LogEntry::Output {
        header: "Header".into(),
        message: message.into(),
    }
}

#[test]
fn entry_heights_count_header_body_and_spacing() {
    assert_eq!(LogEntry::Info("x".into()).height(), 3);
    assert_eq!(
        LogEntry::Action {
            action: "a".into(),
            command: "c".into()
        }
        .height(),
        3
    );
    assert_eq!(output("one").height(), 3);
    assert_eq!(output("one\ntwo\nthree").height(), 5);
    assert_eq!(output("").height(), 2);
}

#[test]
fn pushing_follows_the_last_line() {
    let mut log = LogState::default();
    log.push(LogEntry::Info("first".into()));
    assert_eq!(log.list.selected(), Some(2));
    log.push(output("a\nb"));
    assert_eq!(log.list.selected(), Some(2 + 4));
    assert_eq!(log.entries.len(), 2);
}

#[test]
fn scrolling_wraps_over_lines() {
    let mut log = LogState::default();
    log.scroll(true);
    assert_eq!(log.list.selected(), None, "an empty log doesn't scroll");

    log.push(LogEntry::Info("only".into()));
    log.scroll(true);
    assert_eq!(log.list.selected(), Some(0));
    log.scroll(false);
    assert_eq!(log.list.selected(), Some(2));

    log.list = ListState::default();
    log.scroll(false);
    assert_eq!(log.list.selected(), Some(0));
}

/// One test, because the sink can only be installed once per process.
#[test]
fn the_installed_sink_receives_every_kind_of_entry() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    log::init(move |entry| sink.lock().unwrap().push(entry));
    log::init(|_| panic!("a second sink is ignored"));

    log::action("Build", "nix build");
    log::output("Out", "done");
    log::info("hello");

    let received = received.lock().unwrap();
    let summary: Vec<String> = received
        .iter()
        .map(|e| match e {
            LogEntry::Action { action, command } => format!("action {action}: {command}"),
            LogEntry::Output { header, message } => format!("output {header}: {message}"),
            LogEntry::Info(message) => format!("info {message}"),
        })
        .collect();
    assert_eq!(
        summary,
        ["action Build: nix build", "output Out: done", "info hello"]
    );
}
