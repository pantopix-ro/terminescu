//! Runs a program with a time limit, reading its output on threads so a full pipe never
//! stalls it. Shared by the git and gh strategies.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How long to wait for the pipes to close after the program exits. A hook's background
/// process can keep them open; what the program wrote is in by then.
const DRAIN: Duration = Duration::from_millis(250);

pub struct Output {
    pub ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    NotFound,
    Timeout,
    Io(String),
}

/// Runs `cmd` with no input and kills it after `limit`.
pub fn run(cmd: &mut Command, limit: Duration) -> Result<Output, RunError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RunError::NotFound,
        _ => RunError::Io(e.to_string()),
    })?;
    let out = drain(child.stdout.take().expect("piped"));
    let err = drain(child.stderr.take().expect("piped"));
    let end = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= end => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Err(RunError::Io(e.to_string())),
        }
    };
    let until = Instant::now() + DRAIN;
    Ok(Output {
        ok: status.success(),
        stdout: out.take(until),
        stderr: String::from_utf8_lossy(&err.take(until)).trim().to_string(),
    })
}

/// A pipe read on its own thread until it closes.
struct Drain {
    bytes: Arc<Mutex<Vec<u8>>>,
    done: mpsc::Receiver<()>,
}

fn drain(mut pipe: impl Read + Send + 'static) -> Drain {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let (tx, done) = mpsc::channel();
    let sink = bytes.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            sink.lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend_from_slice(&chunk[..n]);
        }
        let _ = tx.send(());
    });
    Drain { bytes, done }
}

impl Drain {
    /// What came in by `until`, closed or not.
    fn take(self, until: Instant) -> Vec<u8> {
        let _ = self
            .done
            .recv_timeout(until.saturating_duration_since(Instant::now()));
        std::mem::take(&mut *self.bytes.lock().unwrap_or_else(|p| p.into_inner()))
    }
}
