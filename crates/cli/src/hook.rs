//! `terminescu hook`: Claude Code runs it for every hook event in every session on the Mac
//! (D34). It forwards the event to the app and gets out of the way.

use crate::common::{core_socket, pane_id};
use std::io::Read;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use terminescu_protocol::core::{CoreMessage, CoreRequest, post};

/// The whole call, well under Claude's 100 ms budget for us.
const BUDGET: Duration = Duration::from_millis(80);
/// Longest wait for Claude to finish writing the event to stdin.
const STDIN_WAIT: Duration = Duration::from_millis(50);
/// Hook events carry tool input, which can be a whole file; past this the event is dropped.
const MAX_EVENT: u64 = 4 * 1024 * 1024;

pub fn run() -> ExitCode {
    let start = Instant::now();
    // Outside a Terminescu pane there is nobody to tell.
    let Some(pane) = pane_id() else {
        return ExitCode::SUCCESS;
    };
    let (tx, rx) = mpsc::channel();
    // Reading stdin can block on a writer that never closes it; the process exits without
    // waiting for this thread.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = std::io::stdin()
            .lock()
            .take(MAX_EVENT + 1)
            .read_to_end(&mut bytes);
        let _ = tx.send(read.map(|_| bytes));
    });
    let Ok(Ok(bytes)) = rx.recv_timeout(STDIN_WAIT) else {
        return ExitCode::SUCCESS;
    };
    if bytes.len() as u64 > MAX_EVENT {
        return ExitCode::SUCCESS;
    }
    let Ok(event) = serde_json::from_slice(&bytes) else {
        return ExitCode::SUCCESS;
    };
    let left = BUDGET.saturating_sub(start.elapsed());
    if !left.is_zero() {
        // The app may be closed or busy; either way Claude must not notice.
        let _ = post(
            &core_socket(),
            &CoreMessage::new(pane, CoreRequest::Hook { event }),
            left,
        );
    }
    ExitCode::SUCCESS
}
