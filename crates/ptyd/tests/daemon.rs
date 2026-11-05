//! Runs the real `terminescu-ptyd` binary on a socket in a temporary folder.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use terminescu_protocol::client::{Client, ClientError};
use terminescu_protocol::{Control, Frame, PaneId, PaneInfo, SpawnSpec, VarMode};

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Daemon {
    child: Child,
    dir: PathBuf,
    socket: PathBuf,
}

impl Daemon {
    fn start(idle_secs: u64) -> Self {
        // Short folder names: the socket path must stay under 104 bytes.
        let dir = std::env::temp_dir().join(format!(
            "tptyd-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("ptyd.sock");
        let child = Self::spawn(&socket, idle_secs);
        // The file appears at bind, a moment before listen: wait for a connection to work.
        wait_for(
            || std::os::unix::net::UnixStream::connect(&socket).is_ok(),
            "daemon socket",
        );
        Self { child, dir, socket }
    }

    fn spawn(socket: &Path, idle_secs: u64) -> Child {
        Command::new(env!("CARGO_BIN_EXE_terminescu-ptyd"))
            .args([
                "--socket",
                socket.to_str().unwrap(),
                "--idle-exit-secs",
                &idle_secs.to_string(),
            ])
            .env("CLAUDECODE", "1")
            .env("CLAUDE_CODE_ENTRYPOINT", "cli")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn client(&self) -> Client {
        let c = Client::connect(&self.socket, "test").unwrap();
        c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        c
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn wait_for(mut ok: impl FnMut() -> bool, what: &str) {
    let start = Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn(c: &mut Client, argv: &[&str]) -> PaneId {
    let spec = SpawnSpec {
        cols: 80,
        rows: 24,
        argv: Some(argv.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };
    c.send(&Frame::Control(Control::Spawn { req: 7, spec }))
        .unwrap();
    c.recv_until(|f| match f {
        Frame::Control(Control::Spawned { req: 7, pane }) => Some(*pane),
        Frame::Control(Control::Error { message, .. }) => panic!("spawn failed: {message}"),
        _ => None,
    })
    .unwrap()
}

/// Attaches and collects output until `needle` shows up, snapshot included.
fn attach_and_read(c: &mut Client, pane: PaneId, needle: &str) -> String {
    c.send(&Frame::Control(Control::Attach { pane })).unwrap();
    read_until(c, pane, needle)
}

fn read_until(c: &mut Client, pane: PaneId, needle: &str) -> String {
    let mut text = String::new();
    c.recv_until(|f| {
        if let Frame::Output { pane: p, data } = f
            && *p == pane
        {
            text.push_str(&String::from_utf8_lossy(data));
        }
        text.contains(needle).then_some(())
    })
    .unwrap_or_else(|e| panic!("no {needle:?} in {text:?}: {e}"));
    text
}

fn list(c: &mut Client) -> Vec<PaneInfo> {
    c.send(&Frame::Control(Control::List { req: 9 })).unwrap();
    c.recv_until(|f| match f {
        Frame::Control(Control::Panes { req: 9, panes }) => Some(panes.clone()),
        _ => None,
    })
    .unwrap()
}

#[test]
fn shell_survives_a_reconnect_and_comes_back_as_a_snapshot() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(&mut c, &["/bin/sh"]);
    attach_and_read(&mut c, pane, "$");
    c.send(&Frame::Input {
        pane,
        data: b"echo hello-$((40+2))\n".to_vec(),
    })
    .unwrap();
    read_until(&mut c, pane, "hello-42");
    drop(c);

    let mut c = d.client();
    c.send(&Frame::Control(Control::Attach { pane })).unwrap();
    let (cols, rows) = c
        .recv_until(|f| match f {
            Frame::Control(Control::Attached {
                pane: p,
                cols,
                rows,
            }) if *p == pane => Some((*cols, *rows)),
            _ => None,
        })
        .unwrap();
    assert_eq!((cols, rows), (80, 24));
    let snapshot = read_until(&mut c, pane, "hello-42");
    assert!(
        snapshot.starts_with("\x1bc"),
        "a snapshot starts with RIS: {snapshot:?}"
    );

    let info = list(&mut c).into_iter().find(|p| p.pane == pane).unwrap();
    assert!(info.pid.is_some());
    assert!(info.foreground.is_some());
    assert_eq!(info.exited, None);

    // The foreground process follows the job the shell runs.
    c.send(&Frame::Input {
        pane,
        data: b"sleep 5\n".to_vec(),
    })
    .unwrap();
    wait_for(
        || {
            list(&mut c)
                .into_iter()
                .any(|p| p.pane == pane && p.foreground.as_deref() == Some("sleep"))
        },
        "sleep in the foreground",
    );
}

#[test]
fn pane_environment_drops_claude_and_names_the_pane() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(
        &mut c,
        &[
            "/bin/bash",
            "-c",
            "env; yes | head -1 >/dev/null; echo sigpipe=${PIPESTATUS[0]}; echo END",
        ],
    );
    let out = attach_and_read(&mut c, pane, "END\r\n");
    assert!(!out.contains("CLAUDE"), "{out}");
    assert!(
        out.contains(&format!("TERMINESCU_PANE_ID={pane}\r\n")),
        "{out}"
    );
    assert!(
        out.contains(&format!("TERMINESCU_PTYD_SOCKET={}", d.socket.display())),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "TERMINESCU_CORE_SOCKET={}",
            d.dir.join("core.sock").display()
        )),
        "{out}"
    );
    assert!(out.contains("TERM=xterm-256color\r\n"), "{out}");
    assert!(out.contains("TERM_PROGRAM=Terminescu\r\n"), "{out}");
    // Colour is on by default, unless the environment already decides it.
    if std::env::var_os("CLICOLOR").is_none() && std::env::var_os("NO_COLOR").is_none() {
        assert!(out.contains("CLICOLOR=1\r\n"), "{out}");
    }
    // The shell gets SIGPIPE at its default, so a pipeline ends the way it does elsewhere.
    assert!(out.contains("sigpipe=141"), "{out}");
}

#[test]
fn exit_code_is_reported_and_the_pane_stays_listed_until_closed() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(&mut c, &["/bin/sh", "-c", "exit 3"]);
    let code = c
        .recv_until(|f| match f {
            Frame::Control(Control::Exited { pane: p, code }) if *p == pane => Some(*code),
            _ => None,
        })
        .unwrap();
    assert_eq!(code, Some(3));
    assert_eq!(
        list(&mut c).iter().find(|p| p.pane == pane).unwrap().exited,
        Some(3)
    );
    c.send(&Frame::Control(Control::Close { pane })).unwrap();
    assert!(list(&mut c).iter().all(|p| p.pane != pane));
}

#[test]
fn resize_reaches_the_shell() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(&mut c, &["/bin/sh"]);
    attach_and_read(&mut c, pane, "$");
    c.send(&Frame::Control(Control::Resize {
        pane,
        cols: 100,
        rows: 30,
    }))
    .unwrap();
    c.send(&Frame::Input {
        pane,
        data: b"stty size\n".to_vec(),
    })
    .unwrap();
    read_until(&mut c, pane, "30 100");
    let info = list(&mut c).into_iter().find(|p| p.pane == pane).unwrap();
    assert_eq!((info.cols, info.rows), (100, 30));
}

#[test]
fn side_tab_is_listed() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(&mut c, &["/bin/sh"]);
    c.send(&Frame::Control(Control::SetSideTab {
        pane,
        side_tab: "st-1".into(),
    }))
    .unwrap();
    assert_eq!(
        list(&mut c)
            .into_iter()
            .find(|p| p.pane == pane)
            .unwrap()
            .side_tab
            .as_deref(),
        Some("st-1")
    );
}

#[test]
fn unknown_protocol_version_is_refused() {
    let d = Daemon::start(60);
    let mut raw = std::os::unix::net::UnixStream::connect(&d.socket).unwrap();
    use std::io::{Read, Write};
    raw.write_all(&terminescu_protocol::encode(&Frame::Control(
        Control::Hello {
            versions: vec![99],
            client: "future".into(),
            features: vec![],
        },
    )))
    .unwrap();
    raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = Vec::new();
    raw.read_to_end(&mut buf).unwrap();
    let (frame, _) = terminescu_protocol::decode(&buf).unwrap().unwrap();
    assert!(
        matches!(frame, Frame::Control(Control::Error { ref code, .. }) if code == "version"),
        "{frame:?}"
    );
}

#[test]
fn hello_must_come_first() {
    let d = Daemon::start(60);
    let mut raw = std::os::unix::net::UnixStream::connect(&d.socket).unwrap();
    use std::io::{Read, Write};
    raw.write_all(&terminescu_protocol::encode(&Frame::Control(
        Control::List { req: 1 },
    )))
    .unwrap();
    raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = Vec::new();
    raw.read_to_end(&mut buf).unwrap();
    let (frame, _) = terminescu_protocol::decode(&buf).unwrap().unwrap();
    assert!(
        matches!(frame, Frame::Control(Control::Error { ref code, .. }) if code == "hello_first"),
        "{frame:?}"
    );
}

#[test]
fn a_second_daemon_on_the_same_socket_steps_aside() {
    let d = Daemon::start(60);
    let mut second = Daemon::spawn(&d.socket, 60);
    let status = second.wait().unwrap();
    assert!(status.success());
    // The first one still serves.
    let mut c = d.client();
    assert!(list(&mut c).is_empty());
}

#[test]
fn idle_daemon_exits_and_removes_its_socket() {
    let mut d = Daemon::start(1);
    // A connected client keeps it alive.
    let c = d.client();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        d.child.try_wait().unwrap().is_none(),
        "exited with a client connected"
    );
    drop(c);
    wait_for(|| d.child.try_wait().unwrap().is_some(), "idle exit");
    assert!(!d.socket.exists());
    assert!(matches!(
        Client::connect(&d.socket, "late"),
        Err(ClientError::Io(_))
    ));
}

#[test]
fn default_program_is_the_login_shell() {
    let d = Daemon::start(60);
    let mut c = d.client();
    c.send(&Frame::Control(Control::Spawn {
        req: 1,
        spec: SpawnSpec {
            cols: 80,
            rows: 24,
            env: vec![("SHELL".into(), "/bin/zsh".into())],
            ..Default::default()
        },
    }))
    .unwrap();
    let pane = c
        .recv_until(|f| match f {
            Frame::Control(Control::Spawned { req: 1, pane }) => Some(*pane),
            _ => None,
        })
        .unwrap();
    c.send(&Frame::Control(Control::Attach { pane })).unwrap();
    // The integration starts zsh with -l, so it stays a login shell (D8, D9).
    c.send(&Frame::Input {
        pane,
        data: b"[[ -o login ]] && echo login:yes:$((40+2))\n".to_vec(),
    })
    .unwrap();
    let out = read_until(&mut c, pane, ":42\r\n");
    assert!(out.contains("login:yes:42"), "{out}");
}

fn var_call(c: &mut Client, frame: Control) -> Control {
    c.send(&Frame::Control(frame)).unwrap();
    c.recv_until(|f| match f {
        Frame::Control(
            a @ (Control::Vars { req: 9, .. } | Control::Done { req: 9 } | Control::Error { .. }),
        ) => Some(a.clone()),
        _ => None,
    })
    .unwrap()
}

fn var_values(c: &mut Client, pane: PaneId) -> Vec<(String, String)> {
    match var_call(
        c,
        Control::VarGet {
            req: 9,
            pane,
            name: None,
        },
    ) {
        Control::Vars { vars, .. } => vars.into_iter().map(|v| (v.name, v.value)).collect(),
        other => panic!("unexpected {other:?}"),
    }
}

/// D21: a secret outlives the client that set it and is gone once its TTL passes.
#[test]
fn a_secret_outlives_its_client_and_expires_after_its_ttl() {
    let d = Daemon::start(60);
    let mut c = d.client();
    let pane = spawn(&mut c, &["/bin/sleep", "60"]);
    c.send(&Frame::Control(Control::SetSideTab {
        pane,
        side_tab: "st-1".into(),
    }))
    .unwrap();
    let set = |name: &str, ttl_secs| Control::VarSet {
        req: 9,
        pane,
        name: name.into(),
        value: format!("{name}-value"),
        mode: VarMode::Secret,
        ttl_secs,
    };
    assert_eq!(
        var_call(&mut c, set("API_TOKEN", Some(1))),
        Control::Done { req: 9 }
    );
    assert_eq!(
        var_call(&mut c, set("KEEP", None)),
        Control::Done { req: 9 }
    );
    drop(c);

    let mut c = d.client();
    assert_eq!(
        var_values(&mut c, pane),
        [
            ("API_TOKEN".into(), "API_TOKEN-value".into()),
            ("KEEP".into(), "KEEP-value".into())
        ]
    );
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(
        var_values(&mut c, pane),
        [("KEEP".into(), "KEEP-value".into())]
    );

    // A pane with no side tab yet has nowhere to keep variables.
    let loose = spawn(&mut c, &["/bin/sleep", "60"]);
    match var_call(
        &mut c,
        Control::VarGet {
            req: 9,
            pane: loose,
            name: None,
        },
    ) {
        Control::Error { code, .. } => assert_eq!(code, "no_side_tab"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_client_that_asks_gets_shell_pushes_with_run_counters() {
    let d = Daemon::start(60);
    let mut c =
        Client::connect_with(&d.socket, "test", &[terminescu_protocol::FEATURE_SHELL]).unwrap();
    assert_eq!(c.features, [terminescu_protocol::FEATURE_SHELL]);
    let plain = Client::connect_with(&d.socket, "test", &["telepathy"]).unwrap();
    assert!(plain.features.is_empty(), "unknown features are not echoed");
    c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    // Two runs in one write: the counters still show both.
    let script = r"printf '\033]633;E;pnpm test\007\033]133;C;src=terminescu\007\033]133;D;1;src=terminescu\007\033]133;C;src=terminescu\007\033]133;D;3;src=terminescu\007'; sleep 5";
    let pane = spawn(&mut c, &["/bin/sh", "-c", script]);
    let shell = c
        .recv_until(|f| match f {
            Frame::Control(Control::Shell { pane: p, shell }) if *p == pane && shell.ended == 2 => {
                Some(shell.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(shell.started, 2);
    assert_eq!(shell.last_exit, Some(3));
    assert_eq!(shell.command.as_deref(), Some("pnpm test"));
    assert!(!shell.running);

    // A client that did not ask never sees one.
    let mut quiet = d.client();
    quiet
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    spawn(&mut quiet, &["/bin/sh", "-c", script]);
    let got =
        quiet.recv_until(|f| matches!(f, Frame::Control(Control::Shell { .. })).then_some(()));
    assert!(got.is_err(), "no shell push without the feature");
}

/// D21: the app reaches a side tab's variables without a pane, never reads a secret back, and
/// hears about every change, including an expiry, without a value in the push.
#[test]
fn the_app_lists_masked_variables_and_hears_each_change() {
    let d = Daemon::start(60);
    let mut app =
        Client::connect_with(&d.socket, "test", &[terminescu_protocol::FEATURE_VARS]).unwrap();
    assert_eq!(app.features, [terminescu_protocol::FEATURE_VARS]);
    app.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let changed = |c: &mut Client| {
        c.recv_until(|f| match f {
            Frame::Control(Control::VarsChanged { side_tab }) => Some(side_tab.clone()),
            _ => None,
        })
        .unwrap()
    };
    let set = Control::SideVarSet {
        req: 9,
        side_tab: "st-2".into(),
        name: "API_TOKEN".into(),
        value: "tok-example".into(),
        mode: VarMode::Secret,
        ttl_secs: Some(3),
    };
    assert_eq!(var_call(&mut app, set), Control::Done { req: 9 });
    assert_eq!(changed(&mut app), "st-2");
    let listed = var_call(
        &mut app,
        Control::SideVarList {
            req: 9,
            side_tab: "st-2".into(),
        },
    );
    let Control::Vars { vars, .. } = listed else {
        panic!("unexpected {listed:?}");
    };
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].value, "", "the list never carries a secret");
    assert!(vars[0].expires_in_ms.is_some());
    // The TTL passes with nobody reading: the sweep still tells the app.
    assert_eq!(changed(&mut app), "st-2");

    let session = Control::SideVarSet {
        req: 9,
        side_tab: "st-2".into(),
        name: "from_date".into(),
        value: "2026-10-01".into(),
        mode: VarMode::Session,
        ttl_secs: None,
    };
    assert_eq!(var_call(&mut app, session), Control::Done { req: 9 });
    assert_eq!(changed(&mut app), "st-2");
    let dropped = Control::SideVarsDrop {
        req: 9,
        side_tab: "st-2".into(),
    };
    assert_eq!(var_call(&mut app, dropped), Control::Done { req: 9 });
    assert_eq!(changed(&mut app), "st-2");
    let Control::Vars { vars, .. } = var_call(
        &mut app,
        Control::SideVarList {
            req: 9,
            side_tab: "st-2".into(),
        },
    ) else {
        panic!("no list");
    };
    assert!(vars.is_empty());
}
