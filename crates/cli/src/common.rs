//! What the subcommands share: the pane, the sockets, durations and usage errors.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use terminescu_protocol::PaneId;
use terminescu_protocol::paths::{self, CORE_SOCKET_ENV, PANE_ID_ENV, PTYD_SOCKET_ENV};

/// The pane this process runs in, or `None` outside Terminescu.
pub fn pane_id() -> Option<PaneId> {
    std::env::var(PANE_ID_ENV).ok()?.parse().ok()
}

/// The socket from the pane's environment, else the one every Terminescu process agrees on
/// (D36).
fn socket(env: &str, fallback: impl FnOnce(paths::RunPaths) -> PathBuf) -> PathBuf {
    std::env::var_os(env)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback(paths::run_paths()))
}

pub fn core_socket() -> PathBuf {
    socket(CORE_SOCKET_ENV, |p| p.core_socket)
}

pub fn ptyd_socket() -> PathBuf {
    socket(PTYD_SOCKET_ENV, |p| p.ptyd_socket)
}

pub fn usage() -> ExitCode {
    eprintln!("{}", crate::USAGE);
    ExitCode::from(2)
}

pub fn usage_error(message: &str) -> ExitCode {
    eprintln!("terminescu: {message}");
    usage()
}

/// Warns on stderr and still succeeds: for calls a script should never fail on, such as a
/// status while the app is closed (D36).
pub fn warn(message: &str) -> ExitCode {
    eprintln!("terminescu: {message}");
    ExitCode::SUCCESS
}

pub fn fail(message: &str) -> ExitCode {
    eprintln!("terminescu: {message}");
    ExitCode::FAILURE
}

/// `90`, `90s`, `20m`, `1h30m`, `2d`. A bare number means seconds.
pub fn parse_duration(text: &str) -> Option<Duration> {
    if text.is_empty() {
        return None;
    }
    if let Ok(secs) = text.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let mut total: u64 = 0;
    let mut digits = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let unit = match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86_400,
            _ => return None,
        };
        let n: u64 = std::mem::take(&mut digits).parse().ok()?;
        total = total.checked_add(n.checked_mul(unit)?)?;
    }
    (digits.is_empty() && total > 0).then(|| Duration::from_secs(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("1s"), Some(Duration::from_secs(1)));
        assert_eq!(parse_duration("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_duration("20m"), Some(Duration::from_secs(1200)));
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("2d"), Some(Duration::from_secs(172_800)));
        for bad in ["", "m", "1x", "1h30", "0s", "-1s"] {
            assert_eq!(parse_duration(bad), None, "{bad}");
        }
    }
}
