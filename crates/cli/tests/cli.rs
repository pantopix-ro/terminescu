//! Runs the real `terminescu` binary against a test core and an in-process daemon.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use terminescu_protocol::client::Client;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest, Expiry};
use terminescu_protocol::{Control, Frame, PaneId, SpawnSpec, decode_message, encode_message};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A short temporary folder: socket paths must stay under 104 bytes.
fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tcli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The CLI with a clean Terminescu environment: no pane, sockets in `dir`.
fn cli(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_terminescu"));
    c.args(args)
        .env_remove("TERMINESCU_PANE_ID")
        .env("TERMINESCU_HOME", dir)
        .env("TERMINESCU_CORE_SOCKET", dir.join("core.sock"))
        .env("TERMINESCU_PTYD_SOCKET", dir.join("ptyd.sock"))
        .stdin(Stdio::null());
    c
}

fn run_with_stdin(c: &mut Command, input: &str) -> Output {
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// A stand-in for the app's core socket: takes one message, answers `reply` unless the
/// message is a hook, and hands the message to the test.
fn test_core(dir: &Path, reply: CoreReply) -> mpsc::Receiver<CoreMessage> {
    let listener = UnixListener::bind(dir.join("core.sock")).unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let message = loop {
            if let Some((m, _)) = decode_message::<CoreMessage>(&buf).unwrap() {
                break m;
            }
            let n = stream.read(&mut chunk).unwrap();
            assert!(n > 0, "closed before a whole message");
            buf.extend_from_slice(&chunk[..n]);
        };
        if !matches!(message.request, CoreRequest::Hook { .. }) {
            stream.write_all(&encode_message(&reply)).unwrap();
        }
        tx.send(message).unwrap();
    });
    rx
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn status_reaches_the_core_with_its_expiry() {
    let dir = temp_dir();
    let core = test_core(&dir, CoreReply::Ok);
    let out = cli(&dir, &["status", "Deploying", "--for", "1s"])
        .env("TERMINESCU_PANE_ID", "3")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(out.stderr.is_empty(), "{}", stderr(&out));
    let message = core.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        message,
        CoreMessage::new(
            3,
            CoreRequest::Status {
                label: Some("Deploying".into()),
                color: None,
                expiry: Expiry::After { ms: 1000 },
            }
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn status_warns_and_succeeds_when_the_app_is_closed_or_not_ready() {
    let dir = temp_dir();
    let closed = cli(&dir, &["status", "Deploying", "--until-exit"])
        .env("TERMINESCU_PANE_ID", "3")
        .output()
        .unwrap();
    assert!(closed.status.success());
    assert!(
        stderr(&closed).contains("not running"),
        "{}",
        stderr(&closed)
    );

    let _core = test_core(&dir, CoreReply::error("not_built", "statuses arrive later"));
    let not_built = cli(&dir, &["status", "--clear"])
        .env("TERMINESCU_PANE_ID", "3")
        .output()
        .unwrap();
    assert!(not_built.status.success());
    assert!(stderr(&not_built).contains("statuses arrive later"));

    let usage = cli(&dir, &["status"]).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hook_posts_the_event_silently_and_fast() {
    let dir = temp_dir();
    let core = test_core(&dir, CoreReply::Ok);
    let event = r#"{"hook_event_name":"Notification","session_id":"s-1"}"#;
    let start = Instant::now();
    let out = run_with_stdin(cli(&dir, &["hook"]).env("TERMINESCU_PANE_ID", "5"), event);
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "{:?}",
        start.elapsed()
    );
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let message = core.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(message.pane, 5);
    assert_eq!(
        message.request,
        CoreRequest::Hook {
            event: serde_json::from_str(event).unwrap()
        }
    );

    // No app, or junk on stdin: still exit 0 with nothing on stdout.
    let closed = run_with_stdin(
        cli(&dir, &["hook"]).env("TERMINESCU_PANE_ID", "5"),
        "not json",
    );
    assert!(closed.status.success());
    assert!(closed.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A real daemon in this process, with one pane on side tab `st-1`.
struct Daemon {
    dir: PathBuf,
    pane: PaneId,
}

impl Daemon {
    fn start() -> Self {
        let dir = temp_dir();
        let socket = dir.join("ptyd.sock");
        let opts = terminescu_ptyd::server::Options {
            lock: dir.join("ptyd.lock"),
            core_socket: dir.join("core.sock"),
            socket: socket.clone(),
            cli: Some(env!("CARGO_BIN_EXE_terminescu").into()),
            idle: Duration::from_secs(60),
        };
        std::thread::spawn(move || terminescu_ptyd::server::run(opts));
        let start = Instant::now();
        while std::os::unix::net::UnixStream::connect(&socket).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10), "daemon socket");
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut c = Client::connect(&socket, "test").unwrap();
        c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let spec = SpawnSpec {
            cols: 80,
            rows: 24,
            argv: Some(vec!["/bin/sleep".into(), "60".into()]),
            ..Default::default()
        };
        c.send(&Frame::Control(Control::Spawn { req: 7, spec }))
            .unwrap();
        let pane = c
            .recv_until(|f| match f {
                Frame::Control(Control::Spawned { req: 7, pane }) => Some(*pane),
                Frame::Control(Control::Error { message, .. }) => panic!("spawn: {message}"),
                _ => None,
            })
            .unwrap();
        c.send(&Frame::Control(Control::SetSideTab {
            pane,
            side_tab: "st-1".into(),
        }))
        .unwrap();
        // The daemon handles one client's frames in order: a list answered means the side
        // tab is set.
        c.send(&Frame::Control(Control::List { req: 8 })).unwrap();
        c.recv_until(|f| matches!(f, Frame::Control(Control::Panes { req: 8, .. })).then_some(()))
            .unwrap();
        Self { dir, pane }
    }

    fn cli(&self, args: &[&str]) -> Command {
        let mut c = cli(&self.dir, args);
        c.env("TERMINESCU_PANE_ID", self.pane.to_string());
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cli(args).output().unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn variables_live_in_the_daemon_while_the_app_is_closed() {
    let d = Daemon::start();
    // No core socket exists in `d.dir`: the app is closed.
    let set = d.run(&["var", "set", "TICKET", "PROJ-482", "--session"]);
    assert!(set.status.success(), "{}", stderr(&set));
    let secret = run_with_stdin(
        &mut d.cli(&["var", "set", "API_TOKEN", "--secret", "--ttl", "1h"]),
        "tok-example-1\n",
    );
    assert!(secret.status.success(), "{}", stderr(&secret));

    let get = d.run(&["var", "get", "TICKET"]);
    assert_eq!(stdout(&get), "PROJ-482\n");

    let hidden = d.run(&["var", "get", "API_TOKEN"]);
    assert_eq!(hidden.status.code(), Some(1));
    assert!(hidden.stdout.is_empty());
    assert!(stderr(&hidden).contains("--reveal"));
    let revealed = d.run(&["var", "get", "API_TOKEN", "--reveal"]);
    assert_eq!(stdout(&revealed), "tok-example-1\n");

    let env = d.run(&["var", "env"]);
    assert!(env.status.success());
    let env = stdout(&env);
    let lines: Vec<&str> = env.lines().collect();
    assert_eq!(
        lines[..3],
        [
            "export API_TOKEN='tok-example-1'",
            "export TICKET='PROJ-482'",
            "export TERMINESCU_VARS='API_TOKEN TICKET'",
        ]
    );
    let key = lines[3]
        .strip_prefix("export TERMINESCU_VARS_GEN='")
        .and_then(|k| k.strip_suffix('\''))
        .unwrap()
        .to_string();
    // A shell that already loaded this generation gets nothing to eval.
    let same = d
        .cli(&["var", "env"])
        .env("TERMINESCU_VARS_GEN", &key)
        .output()
        .unwrap();
    assert!(same.status.success() && same.stdout.is_empty());

    let list = d.run(&["var", "list"]);
    assert_eq!(
        stdout(&list),
        "API_TOKEN\tsecret\t(hidden)\nTICKET\tsession\tPROJ-482\n"
    );

    // A saved variable needs the app: warn, succeed, keep nothing.
    let saved = d.run(&["var", "set", "from_date", "2026-11-01"]);
    assert!(saved.status.success());
    assert!(stderr(&saved).contains("not running"), "{}", stderr(&saved));

    assert!(d.run(&["var", "unset", "TICKET"]).status.success());
    let gone = d.run(&["var", "get", "TICKET"]);
    assert_eq!(gone.status.code(), Some(1));
    // The shell still holds TICKET from the last load: it gets an unset line.
    let after = d
        .cli(&["var", "env", "--fish"])
        .env("TERMINESCU_VARS", "API_TOKEN TICKET")
        .env("TERMINESCU_VARS_GEN", &key)
        .output()
        .unwrap();
    let after = stdout(&after);
    assert!(
        after.starts_with("set -e TICKET\nset -gx API_TOKEN 'tok-example-1'\n"),
        "{after}"
    );
}

#[test]
fn var_env_prints_nothing_without_a_pane_or_a_daemon() {
    let dir = temp_dir();
    let no_pane = cli(&dir, &["var", "env"]).output().unwrap();
    assert!(no_pane.status.success());
    assert!(no_pane.stdout.is_empty() && no_pane.stderr.is_empty());
    let no_daemon = cli(&dir, &["var", "env"])
        .env("TERMINESCU_PANE_ID", "1")
        .output()
        .unwrap();
    assert!(no_daemon.status.success());
    assert!(no_daemon.stdout.is_empty() && no_daemon.stderr.is_empty());
    let get = cli(&dir, &["var", "get", "X"])
        .env("TERMINESCU_PANE_ID", "1")
        .output()
        .unwrap();
    assert_eq!(get.status.code(), Some(1));
    assert!(
        stderr(&get).contains("daemon is not running"),
        "{}",
        stderr(&get)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
