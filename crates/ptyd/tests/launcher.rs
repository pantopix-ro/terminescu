//! The core's launcher starts the real daemon detached and finds it again (D32).

use std::time::Duration;
use terminescu_core::launcher::{ensure_running, install, is_running};
use terminescu_protocol::client::Client;

#[test]
fn launcher_starts_one_daemon_and_reuses_it() {
    let dir = std::env::temp_dir().join(format!("tl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let installed = install(
        env!("CARGO_BIN_EXE_terminescu-ptyd").as_ref(),
        &dir.join("bin"),
    )
    .unwrap();
    let socket = dir.join("ptyd.sock");
    let log = dir.join("ptyd.log");
    // A short idle limit, so the detached daemon cleans up after the test.
    let extra = ["--idle-exit-secs", "1"];
    ensure_running(&installed, &socket, &log, &extra, Duration::from_secs(10)).unwrap();
    assert!(is_running(&socket));
    let c = Client::connect(&socket, "test").unwrap();
    assert_eq!(c.version, 1);
    // A second call finds the running daemon instead of starting another.
    ensure_running(&installed, &socket, &log, &extra, Duration::from_secs(10)).unwrap();
    drop(c);
    let start = std::time::Instant::now();
    while socket.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "daemon did not exit when idle"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
