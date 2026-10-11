use nui::backend::Error;
use nui::backend::process::{ChildRegistry, cancel_registered, stdout};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn cmd(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd
}

#[test]
fn captures_stdout_without_a_registry() {
    let out = stdout("echo", cmd("echo", &["hello"]), None).unwrap();
    assert_eq!(String::from_utf8_lossy(&out).trim(), "hello");
}

#[test]
fn captures_stdout_with_a_registry() {
    let reg = ChildRegistry::default();
    let out = stdout("echo", cmd("echo", &["hello"]), Some(&reg)).unwrap();
    assert_eq!(String::from_utf8_lossy(&out).trim(), "hello");
}

#[test]
fn a_failing_command_reports_its_status() {
    let err = stdout("sh", cmd("sh", &["-c", "echo partial; exit 3"]), None).unwrap_err();
    match err {
        Error::Failed("sh", status) => assert_eq!(status.code(), Some(3)),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn a_missing_program_is_a_spawn_error() {
    let err = stdout("nope", cmd("nui-test-no-such-binary", &[]), None).unwrap_err();
    assert!(matches!(err, Error::Spawn("nope", _)));
    assert!(err.to_string().starts_with("Failed to execute nope"));
}

/// A registered child must be killed and reaped when a newer search cancels it, and the
/// waiting caller must return promptly instead of blocking for the child's full runtime.
#[test]
fn cancel_registered_kills_an_in_flight_child() {
    let reg = ChildRegistry::default();
    let worker_reg = Arc::clone(&reg);

    let started = Instant::now();
    let worker =
        std::thread::spawn(move || stdout("sleep", cmd("sleep", &["30"]), Some(&worker_reg)));

    let deadline = Instant::now() + Duration::from_secs(5);
    while reg.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "child was never registered");
        std::thread::sleep(Duration::from_millis(10));
    }
    cancel_registered(&reg);

    let result = worker.join().expect("worker panicked");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancelled child was not killed promptly"
    );
    match result {
        Err(Error::Failed(..)) => {}
        Err(Error::Spawn(_, e)) => assert_eq!(e.to_string(), "search cancelled"),
        other => panic!("cancelled child should fail, got {:?}", other.map(|_| ())),
    }
    assert!(reg.lock().unwrap().is_empty(), "registry was not drained");
}

#[test]
fn cancelling_an_empty_registry_is_harmless() {
    let reg = ChildRegistry::default();
    cancel_registered(&reg);
    assert!(reg.lock().unwrap().is_empty());
}
