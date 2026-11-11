//! Timing helpers shared by the CLI tests.

use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// D34's budget for the hook's own work.
pub const BUDGET: Duration = Duration::from_millis(100);

/// What starting the binary costs on this machine, outside the hook's control: the slowest of
/// three runs that return at once (no pane). The first, untimed start pays for macOS checking
/// a freshly built binary. A loaded CI machine can take tens of ms for this alone.
pub fn spawn_cost() -> Duration {
    static COST: OnceLock<Duration> = OnceLock::new();
    *COST.get_or_init(|| {
        let run = || {
            let start = Instant::now();
            let _ = Command::new(env!("CARGO_BIN_EXE_terminescu"))
                .arg("hook")
                .env_remove("TERMINESCU_PANE_ID")
                .stdin(Stdio::null())
                .output();
            start.elapsed()
        };
        run();
        (0..3).map(|_| run()).max().unwrap_or_default()
    })
}
