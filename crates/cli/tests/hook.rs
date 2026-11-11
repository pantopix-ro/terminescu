use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// D34's budget for the hook's own work.
const BUDGET: Duration = Duration::from_millis(100);

/// What starting the binary costs on this machine, outside the hook's control: the slowest of
/// three runs that return at once (no pane). The first, untimed start pays for macOS checking
/// a freshly built binary. A loaded CI machine can take tens of ms for this alone.
fn spawn_cost() -> Duration {
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

/// The hook must never get in Claude's way: exit 0, empty stdout, well under 100 ms (D34).
#[test]
fn hook_exits_zero_silently_and_fast() {
    let budget = BUDGET + spawn_cost();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_terminescu"))
        .arg("hook")
        .env_remove("TERMINESCU_PANE_ID")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert!(
        start.elapsed() < budget,
        "{:?} of {budget:?}",
        start.elapsed()
    );
}

fn hook_in_pane(socket: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_terminescu"));
    c.arg("hook")
        .env("TERMINESCU_PANE_ID", "7")
        .env("TERMINESCU_CORE_SOCKET", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

/// Inside a pane but with the app quit: the socket is missing, and Claude still notices nothing.
#[test]
fn hook_exits_zero_silently_and_fast_without_the_socket() {
    use std::io::Write;
    let socket =
        std::env::temp_dir().join(format!("terminescu-no-core-{}.sock", std::process::id()));
    let budget = BUDGET + spawn_cost();
    let start = Instant::now();
    let mut child = hook_in_pane(&socket).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"hook_event_name":"Stop","session_id":"s-1"}"#)
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert!(
        start.elapsed() < budget,
        "{:?} of {budget:?}",
        start.elapsed()
    );
}

/// A writer that never closes stdin must not hold the hook: it gives up after its 50 ms wait.
/// The bound is loose on purpose. That wait is a timer, and macOS may fire a child's timers
/// late: tens of ms on a Mac, about 100 ms on a loaded CI runner. Claude always closes stdin,
/// so the paths it takes, timed above, stay under the strict budget.
#[test]
fn hook_gives_up_on_a_stdin_that_never_closes() {
    let socket =
        std::env::temp_dir().join(format!("terminescu-no-core-{}-b.sock", std::process::id()));
    let start = Instant::now();
    let mut child = hook_in_pane(&socket).spawn().unwrap();
    let stdin = child.stdin.take().unwrap();
    let status = child.wait().unwrap();
    drop(stdin);
    assert!(status.success());
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
}

/// `SessionStart` adds the `var env` line to Claude's env file once, never a value (D35).
#[test]
fn session_start_adds_the_env_line_once() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("terminescu-envfile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("env.sh");
    std::fs::write(&file, "export EXISTING=1\n").unwrap();
    let socket = dir.join("no-core.sock");
    for _ in 0..2 {
        let mut child = hook_in_pane(&socket)
            .env("CLAUDE_ENV_FILE", &file)
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"hook_event_name":"SessionStart","session_id":"sess-0b5c"}"#)
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success() && out.stdout.is_empty());
    }
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("export EXISTING=1\n"), "{text}");
    assert_eq!(text.matches(" var env ").count(), 1, "{text}");
    assert!(text.contains("eval \"$('"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
