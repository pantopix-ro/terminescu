//! `terminescu hook`: Claude Code runs it for every hook event in every session on the Mac
//! (D34). It forwards the event to the app and gets out of the way.
//!
//! With `--baseline-*` options it forwards nothing and instead keeps a first-touch copy of the
//! file the event names (add-claude-insights D3). The options are JSON pointers into the event,
//! written by the plugin that installs the hook, so this program knows no agent's payload.

use crate::common::{core_socket, pane_id};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use terminescu_plugin_api::baseline;
use terminescu_protocol::core::{CoreMessage, CoreRequest, post};
use terminescu_protocol::paths;

/// The whole call, well under Claude's 100 ms budget for us.
const BUDGET: Duration = Duration::from_millis(80);
/// Longest wait for Claude to finish writing the event to stdin.
const STDIN_WAIT: Duration = Duration::from_millis(50);
/// Hook events carry tool input, which can be a whole file; past this the event is dropped.
const MAX_EVENT: u64 = 4 * 1024 * 1024;
/// The file Claude Code sources before each Bash command; `SessionStart` hooks may append to it.
const CLAUDE_ENV_FILE: &str = "CLAUDE_ENV_FILE";

/// Where in the event a baseline copy finds its session, its path and the cwd a relative path
/// joins.
#[derive(Debug, Default)]
struct Baseline {
    session: Option<String>,
    paths: Vec<String>,
    cwd: Option<String>,
}

impl Baseline {
    /// The options, or `None` when there are none. Anything unknown makes it a plain hook.
    fn parse(args: &[String]) -> Option<Self> {
        let mut b = Baseline::default();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let value = it.next()?.clone();
            match flag.as_str() {
                "--baseline-session" => b.session = Some(value),
                "--baseline-path" => b.paths.push(value),
                "--baseline-cwd" => b.cwd = Some(value),
                _ => return None,
            }
        }
        (b.session.is_some() && !b.paths.is_empty()).then_some(b)
    }

    fn copy(&self, event: &Value) {
        let text = |ptr: &Option<String>| ptr.as_deref().and_then(|p| event.pointer(p)?.as_str());
        let Some(session) = text(&self.session) else {
            return;
        };
        let Some(raw) = self.paths.iter().find_map(|p| event.pointer(p)?.as_str()) else {
            return;
        };
        let path = Path::new(raw);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            match text(&self.cwd).map(Path::new).filter(|c| c.is_absolute()) {
                Some(cwd) => cwd.join(path),
                None => return,
            }
        };
        let store = baseline::store(&paths::support_dir());
        let cap = baseline::file_cap(&store);
        let _ = baseline::first_touch(&store, session, &path, cap);
    }
}

pub fn run(args: &[String]) -> ExitCode {
    // A panic must not reach Claude as exit 101 or as text: it ends here, silently.
    std::panic::set_hook(Box::new(|_| {}));
    let baseline = Baseline::parse(args);
    let _ = std::panic::catch_unwind(|| forward(baseline.as_ref()));
    ExitCode::SUCCESS
}

fn forward(baseline: Option<&Baseline>) {
    let start = Instant::now();
    // Outside a Terminescu pane there is nobody to tell.
    let Some(pane) = pane_id() else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    // Reading stdin can block on a writer that never closes it; the process exits without
    // waiting for this thread.
    let reader = std::thread::Builder::new().spawn(move || {
        let mut bytes = Vec::new();
        let read = std::io::stdin()
            .lock()
            .take(MAX_EVENT + 1)
            .read_to_end(&mut bytes);
        let _ = tx.send(read.map(|_| bytes));
    });
    if reader.is_err() {
        return;
    }
    let Ok(Ok(bytes)) = rx.recv_timeout(STDIN_WAIT) else {
        return;
    };
    if bytes.len() as u64 > MAX_EVENT {
        return;
    }
    let Ok(event) = serde_json::from_slice::<Value>(&bytes) else {
        return;
    };
    if let Some(b) = baseline {
        return b.copy(&event);
    }
    if event.get("hook_event_name").and_then(|v| v.as_str()) == Some("SessionStart")
        && let Some(file) = std::env::var_os(CLAUDE_ENV_FILE).filter(|f| !f.is_empty())
    {
        let _ = add_env_line(Path::new(&file));
    }
    let left = BUDGET.saturating_sub(start.elapsed());
    if !left.is_zero() {
        // The app may be closed or busy; either way Claude must not notice.
        let _ = post(
            &core_socket(),
            &CoreMessage::new(pane, CoreRequest::Hook { event }),
            left,
        );
    }
}

/// The line that gives Claude's Bash commands the side tab's session and secret variables
/// (D35). The values never touch the file: Claude runs `var env` each time.
fn env_line(cli: &Path) -> String {
    let quoted = format!("'{}'", cli.to_string_lossy().replace('\'', r"'\''"));
    format!("eval \"$({quoted} var env 2>/dev/null)\"\n")
}

/// Appends [`env_line`] once; a resumed session reuses the same file.
fn add_env_line(file: &Path) -> std::io::Result<()> {
    let line = env_line(&std::env::current_exe()?);
    if std::fs::read_to_string(file).is_ok_and(|text| text.contains(line.trim_end())) {
        return Ok(());
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?
        .write_all(line.as_bytes())
}
