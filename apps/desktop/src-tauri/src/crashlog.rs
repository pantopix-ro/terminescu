//! Keeps the diagnostics of an app started from Finder, whose stderr goes nowhere.
//!
//! Without a terminal, stderr goes to `logs/app.log` in the support folder: every
//! `terminescu:` line and every panic message lands there. A startup error shows an alert
//! and exits, where Tauri would abort with no message.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use terminescu_protocol::paths;

/// The log keeps at most this many bytes; a bigger one moves to `app.log.1` at start.
const MAX_BYTES: u64 = 1024 * 1024;

pub fn log_path() -> PathBuf {
    paths::support_dir().join("logs/app.log")
}

/// Sends stderr to the log when no terminal is attached, and stamps each panic with the time.
pub fn init() {
    // SAFETY: isatty only reads the descriptor.
    if unsafe { libc::isatty(2) } == 0 {
        let _ = redirect();
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("terminescu: panic at {}", now());
        default(info);
    }));
}

fn redirect() -> std::io::Result<()> {
    let path = log_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        std::fs::rename(&path, path.with_extension("log.1"))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(
        file,
        "terminescu: {} started at {}",
        env!("CARGO_PKG_VERSION"),
        now()
    )?;
    // SAFETY: both descriptors are open; dup2 replaces stderr with the log file.
    if unsafe { libc::dup2(file.as_raw_fd(), 2) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Logs a startup error, shows it in an alert and exits.
pub fn fail(error: &dyn std::fmt::Display) -> ! {
    eprintln!("terminescu: start failed: {error}");
    let message = format!("{error}\n\nDetails: {}", log_path().display());
    // osascript runs in its own process, so the alert needs no event loop. The message goes
    // in as an argument, so no quoting is needed.
    let _ = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display alert \"Terminescu could not start\" message (item 1 of argv) as critical",
            "-e",
            "end run",
            &message,
        ])
        .status();
    std::process::exit(1);
}

/// The local time as `YYYY-MM-DD HH:MM:SS`.
fn now() -> String {
    // SAFETY: time and localtime_r write only into the locals passed in.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}
