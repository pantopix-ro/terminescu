//! Runs a program with a time limit, reading its output on threads so a full pipe never
//! stalls it. Shared by the git and gh strategies.
//!
//! The program runs in a process group of its own, so a timeout ends what it started too:
//! `gh pr checkout`'s `git fetch`, a hook, an LFS filter.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How long to wait for the pipes to close after the program exits. A hook's background
/// process can keep them open; what the program wrote is in by then.
const DRAIN: Duration = Duration::from_millis(250);

/// The most stdout a call keeps. The largest reader is a diff base (4 MiB shown); a status
/// of a very large repository stays well under this.
pub const STDOUT_CAP: usize = 32 << 20;
/// The most stderr a call reads; what it keeps after redaction is [`STDERR_KEEP`].
const STDERR_CAP: usize = 64 << 10;
/// The most stderr text passed on to a log line or the UI.
pub const STDERR_KEEP: usize = 2048;

pub struct Output {
    pub ok: bool,
    pub stdout: Vec<u8>,
    /// Trimmed, with credentials in URLs taken out and cut to [`STDERR_KEEP`].
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    NotFound,
    Timeout,
    /// The program wrote more than [`STDOUT_CAP`].
    TooLarge,
    Io(String),
}

/// Runs `cmd` with no input and kills it, and everything it started, after `limit`.
pub fn run(cmd: &mut Command, limit: Duration) -> Result<Output, RunError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RunError::NotFound,
        _ => RunError::Io(e.to_string()),
    })?;
    let out = drain(child.stdout.take().expect("piped"), STDOUT_CAP);
    let err = drain(child.stderr.take().expect("piped"), STDERR_CAP);
    let end = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= end => {
                kill_group(&mut child);
                return Err(RunError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                kill_group(&mut child);
                return Err(RunError::Io(e.to_string()));
            }
        }
    };
    let until = Instant::now() + DRAIN;
    let (stdout, over) = out.take(until);
    if over {
        return Err(RunError::TooLarge);
    }
    Ok(Output {
        ok: status.success(),
        stdout,
        stderr: clean_stderr(&String::from_utf8_lossy(&err.take(until).0)),
    })
}

/// SIGKILL to the child's group (its pid, from `process_group(0)`), then reaps the child.
fn kill_group(child: &mut Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: kill(2) with a negative pid signals that process group and touches no memory.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Stderr as the log and the UI get it: trimmed, `scheme://user:secret@host` shown as
/// `scheme://host`, and at most [`STDERR_KEEP`] bytes.
pub fn clean_stderr(text: &str) -> String {
    let text = redact(text.trim());
    if text.len() <= STDERR_KEEP {
        return text;
    }
    let mut end = STDERR_KEEP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Takes the userinfo out of every URL: a remote URL can carry a token
/// (`https://user:TOKEN@github.com/...`), and git does not always hide it.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        match authority.rfind('@') {
            Some(i) => out.push_str(&authority[i + 1..]),
            None => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// A pipe read on its own thread until it closes. Past `cap` bytes it keeps reading, so the
/// program never blocks on a full pipe, but keeps nothing more.
struct Drain {
    bytes: Arc<Mutex<Vec<u8>>>,
    over: Arc<AtomicBool>,
    done: mpsc::Receiver<()>,
}

fn drain(mut pipe: impl Read + Send + 'static, cap: usize) -> Drain {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let over = Arc::new(AtomicBool::new(false));
    let (tx, done) = mpsc::channel();
    let (sink, full) = (bytes.clone(), over.clone());
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let mut kept = sink.lock().unwrap_or_else(|p| p.into_inner());
            let room = cap.saturating_sub(kept.len());
            if n > room {
                full.store(true, Ordering::Relaxed);
            }
            kept.extend_from_slice(&chunk[..n.min(room)]);
        }
        let _ = tx.send(());
    });
    Drain { bytes, over, done }
}

impl Drain {
    /// What came in by `until`, closed or not, and whether some was dropped at the cap.
    fn take(self, until: Instant) -> (Vec<u8>, bool) {
        let _ = self
            .done
            .recv_timeout(until.saturating_duration_since(Instant::now()));
        let bytes = std::mem::take(&mut *self.bytes.lock().unwrap_or_else(|p| p.into_inner()));
        (bytes, self.over.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_in_urls_are_taken_out() {
        assert_eq!(
            redact("fatal: unable to access 'https://user:not-a-token@github.com/acme/api.git/'"),
            "fatal: unable to access 'https://github.com/acme/api.git/'"
        );
        assert_eq!(
            redact("ssh://git@host.example/x and https://t@a.example"),
            "ssh://host.example/x and https://a.example"
        );
        assert_eq!(redact("no url here"), "no url here");
        assert_eq!(redact("https://github.com/acme"), "https://github.com/acme");
    }

    #[test]
    fn long_stderr_is_cut_on_a_char_boundary() {
        let long = "é".repeat(STDERR_KEEP);
        let cut = clean_stderr(&long);
        assert!(cut.len() <= STDERR_KEEP + "…".len());
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn a_timeout_ends_what_the_program_started() {
        let dir = std::env::temp_dir().join(format!("tsc-pg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("late");
        // The grandchild writes the marker after the limit, unless its group is killed.
        let script = format!("(sleep 0.6; touch '{}') & sleep 5", marker.display());
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", &script]);
        let started = Instant::now();
        assert_eq!(
            run(&mut cmd, Duration::from_millis(150)).err(),
            Some(RunError::Timeout)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(900));
        assert!(!marker.exists(), "the grandchild outlived the timeout");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn output_past_the_cap_is_too_large() {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "head -c 40000 /dev/zero"]);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        let out = drain(child.stdout.take().unwrap(), 10_000);
        child.wait().unwrap();
        let (bytes, over) = out.take(Instant::now() + Duration::from_secs(2));
        assert_eq!(bytes.len(), 10_000);
        assert!(over);
    }
}
