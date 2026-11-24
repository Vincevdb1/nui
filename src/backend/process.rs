use super::{Error, Result};
use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

/// One spawned child process, shared between the thread waiting on it and whoever may
/// cancel it. `None` once the child has been taken for reaping.
pub type ChildSlot = Arc<Mutex<Option<Child>>>;

/// All child processes belonging to the current search generation.
pub type ChildRegistry = Arc<Mutex<Vec<ChildSlot>>>;

/// Every live child, so quitting can kill them all.
static LIVE: Mutex<Vec<Weak<Mutex<Option<Child>>>>> = Mutex::new(Vec::new());
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn kill(slot: &ChildSlot) {
    if let Some(child) = lock(slot).as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Kill and reap every child registered so far, then empty the registry.
pub fn cancel_registered(reg: &ChildRegistry) {
    let slots: Vec<ChildSlot> = std::mem::take(&mut *lock(reg));
    slots.iter().for_each(kill);
}

/// Kills every running child and refuses to start new ones.
pub fn shutdown() {
    let live = {
        let mut live = lock(&LIVE);
        SHUTDOWN.store(true, Ordering::SeqCst);
        std::mem::take(&mut *live)
    };
    live.iter().filter_map(Weak::upgrade).for_each(|s| kill(&s));
}

/// A child that `shutdown` (and the owner of its registry, if any) can kill.
pub struct Tracked {
    slot: ChildSlot,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

impl Tracked {
    pub fn wait(self) -> std::io::Result<ExitStatus> {
        // Release the slot before blocking so a killer never waits on this thread.
        let child = lock(&self.slot).take();
        match child {
            Some(mut child) => child.wait(),
            None => Err(std::io::Error::other("process killed")),
        }
    }
}

pub fn spawn(cmd: &mut Command, reg: Option<&ChildRegistry>) -> std::io::Result<Tracked> {
    let mut live = lock(&LIVE);
    if SHUTDOWN.load(Ordering::SeqCst) {
        return Err(std::io::Error::other("nui is shutting down"));
    }
    let mut child = cmd.spawn()?;
    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    let slot: ChildSlot = Arc::new(Mutex::new(Some(child)));
    live.retain(|w| w.strong_count() > 0);
    live.push(Arc::downgrade(&slot));
    drop(live);
    if let Some(reg) = reg {
        lock(reg).push(Arc::clone(&slot));
    }
    Ok(Tracked {
        slot,
        stdout,
        stderr,
    })
}

/// Like `Command::output`, but killable by `shutdown`.
pub fn output(cmd: &mut Command) -> std::io::Result<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn(cmd, None)?;
    let stderr = child.stderr.take().map(|mut p| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = p.read_to_end(&mut buf);
            buf
        })
    });
    let stdout = read_all(child.stdout.take());
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(Output {
        status: child.wait()?,
        stdout,
        stderr,
    })
}

fn read_all(pipe: Option<ChildStdout>) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(mut p) = pipe {
        let _ = p.read_to_end(&mut buf);
    }
    buf
}

/// Runs `cmd` and returns its stdout, failing on a non-zero exit.
///
/// stderr is discarded rather than captured: every caller ignores it, and it must never be
/// inherited or it would corrupt the TUI.
pub fn stdout(
    tool: &'static str,
    mut cmd: Command,
    reg: Option<&ChildRegistry>,
) -> Result<Vec<u8>> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = spawn(&mut cmd, reg).map_err(|e| Error::Spawn(tool, e))?;
    let out = read_all(child.stdout.take());
    let status = child.wait().map_err(|e| Error::Spawn(tool, e))?;
    if !status.success() {
        return Err(Error::Failed(tool, status));
    }
    Ok(out)
}
