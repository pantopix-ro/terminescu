use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The hook must never get in Claude's way: exit 0, empty stdout, well under 100 ms (D34).
#[test]
fn hook_exits_zero_silently_and_fast() {
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_terminescu"))
        .arg("hook")
        .env_remove("TERMINESCU_PANE_ID")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert!(start.elapsed() < Duration::from_millis(100));
}
