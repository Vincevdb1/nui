use super::{Error, Result};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

/// One spawned child process, shared between the thread waiting on it and whoever may
/// cancel it. `None` once the child has been taken for reaping.
pub type ChildSlot = Arc<Mutex<Option<Child>>>;

/// All child processes belonging to the current search generation.
pub type ChildRegistry = Arc<Mutex<Vec<ChildSlot>>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Kill and reap every child registered so far, then empty the registry.
pub fn cancel_registered(reg: &ChildRegistry) {
    let slots: Vec<ChildSlot> = std::mem::take(&mut *lock(reg));
    for slot in slots {
        let mut guard = lock(&slot);
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Runs `cmd` and returns its stdout, failing on a non-zero exit.
pub fn stdout(tool: &'static str, cmd: Command, reg: Option<&ChildRegistry>) -> Result<Vec<u8>> {
    let output = run_registered(cmd, reg).map_err(|e| Error::Spawn(tool, e))?;
    if !output.status.success() {
        return Err(Error::Failed(tool, output.status));
    }
    Ok(output.stdout)
}

/// Run `cmd` capturing stdout, registering the child so a newer search can kill it.
///
/// stderr is discarded rather than captured: every caller ignores it, and it must never be
/// inherited or it would corrupt the TUI. Discarding it also removes the pipe-fill deadlock
/// that reading two pipes sequentially on one thread would risk.
fn run_registered(
    mut cmd: Command,
    reg: Option<&ChildRegistry>,
) -> std::io::Result<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn()?;

    let Some(reg) = reg else {
        return child.wait_with_output();
    };

    let mut pipe = child.stdout.take();
    let slot: ChildSlot = Arc::new(Mutex::new(Some(child)));
    lock(reg).push(Arc::clone(&slot));

    let mut stdout = Vec::new();
    if let Some(p) = pipe.as_mut() {
        let _ = p.read_to_end(&mut stdout);
    }
    drop(pipe);

    let mut guard = lock(&slot);
    let status = match guard.take() {
        Some(mut child) => child.wait()?,
        // Already reaped by cancel_registered.
        None => return Err(std::io::Error::other("search cancelled")),
    };

    Ok(std::process::Output {
        status,
        stdout,
        stderr: Vec::new(),
    })
}

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use std::time::Instant;

    /// A registered child must be killed and reaped when a newer search cancels it, and the
    /// waiting caller must return promptly instead of blocking for the child's full runtime.
    #[test]
    fn cancel_registered_kills_in_flight_child() {
        let reg = ChildRegistry::default();
        let reg_worker = Arc::clone(&reg);

        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            let mut cmd = Command::new("sleep");
            cmd.arg("30");
            run_registered(cmd, Some(&reg_worker))
        });

        // Wait until the child is actually registered, then cancel it.
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while lock(&reg).is_empty() {
            assert!(Instant::now() < deadline, "child was never registered");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cancel_registered(&reg);

        let result = worker.join().expect("worker panicked");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "cancelled child was not killed promptly"
        );
        match result {
            Ok(output) => assert!(!output.status.success(), "killed child reported success"),
            Err(e) => assert_eq!(e.to_string(), "search cancelled"),
        }
        assert!(lock(&reg).is_empty(), "registry was not drained");
    }

    /// Without a registry the runner still behaves like `Command::output()`.
    #[test]
    fn run_registered_without_registry_captures_stdout() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let out = run_registered(cmd, None).expect("echo failed to run");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }
}
