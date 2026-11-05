//! The core's variable store against a real daemon (D21, D35): one name per side tab across
//! SQLite and the daemon, modes that move a value between them, and a secret that never comes
//! back out or lands on disk.

use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use terminescu_core::context::{ContextError, Mode};
use terminescu_core::daemon::{DaemonClient, DaemonEvent};
use terminescu_core::db::Db;
use terminescu_core::layout::Layout;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};
use terminescu_core::vars::{self, NewVariable, VariableChange};

struct Daemon {
    child: Child,
    dir: std::path::PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn start() -> (Daemon, DaemonClient, mpsc::Receiver<DaemonEvent>) {
    let dir = std::env::temp_dir().join(format!("tctx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("ptyd.sock");
    let child = Command::new(env!("CARGO_BIN_EXE_terminescu-ptyd"))
        .args([
            "--socket",
            socket.to_str().unwrap(),
            "--idle-exit-secs",
            "60",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while std::os::unix::net::UnixStream::connect(&socket).is_err() {
        assert!(started.elapsed() < Duration::from_secs(10), "daemon socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (tx, rx) = mpsc::channel();
    let client = DaemonClient::connect(&socket, tx).unwrap();
    assert!(client.side_vars);
    (Daemon { child, dir }, client, rx)
}

fn db_at(path: &std::path::Path) -> Db {
    let db = Db::open(path).unwrap();
    let workspace = Workspace {
        windows: vec![Window {
            id: "w1".into(),
            side_tabs: vec![SideTab {
                status: None,
                icon: None,
                id: "s1".into(),
                title: Some("PROJ-482".into()),
                tabs: vec![Tab {
                    status: None,
                    icon: None,
                    id: "t1".into(),
                    title: None,
                    layout: Layout::pane("p1"),
                }],
            }],
        }],
    };
    save(
        &db,
        &Saved {
            workspace,
            ..Default::default()
        },
    )
    .unwrap();
    db
}

fn new(name: &str, value: &str, mode: Mode) -> NewVariable {
    NewVariable {
        name: name.into(),
        value: value.into(),
        mode,
        ttl_secs: None,
        source: "typed".into(),
        origin_tab_id: Some("t1".into()),
    }
}

fn on_disk(dir: &std::path::Path, needle: &str) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|e| {
        std::fs::read(e.path())
            .map(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes()))
            .unwrap_or(false)
    })
}

#[test]
fn modes_move_values_between_sqlite_and_the_daemon() {
    let (d, client, events) = start();
    let dbdir = d.dir.join("db");
    std::fs::create_dir_all(&dbdir).unwrap();
    let db = db_at(&dbdir.join("t.sqlite"));
    let daemon = Some(&client);

    vars::add(
        &db,
        daemon,
        "s1",
        &new("ticket", "Add audit export endpoint", Mode::Saved),
        false,
    )
    .unwrap();
    vars::add(
        &db,
        daemon,
        "s1",
        &new("from_date", "2026-10-01", Mode::Session),
        false,
    )
    .unwrap();
    let mut secret = new("API_TOKEN", "tok-example-7f3a", Mode::Secret);
    secret.ttl_secs = Some(3600);
    let s = vars::add(&db, daemon, "s1", &secret, false).unwrap();
    assert_eq!((s.mode, s.value.as_deref()), (Mode::Secret, None));
    assert!(s.expires_at.is_some());
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(5)),
        Ok(DaemonEvent::VarsChanged { side_tab }) if side_tab == "s1"
    ));

    let listed = vars::list(&db, daemon, "s1").unwrap();
    let shown: Vec<(&str, Mode, Option<&str>)> = listed
        .iter()
        .map(|v| (v.name.as_str(), v.mode, v.value.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [
            ("ticket", Mode::Saved, Some("Add audit export endpoint")),
            ("API_TOKEN", Mode::Secret, None),
            ("from_date", Mode::Session, Some("2026-10-01")),
        ]
    );

    // One name per side tab, across both stores.
    assert!(matches!(
        vars::add(
            &db,
            daemon,
            "s1",
            &new("from_date", "x", Mode::Saved),
            false
        ),
        Err(ContextError::NameTaken(..))
    ));
    // A secret never turns back into something that shows or saves its value.
    let to_saved = VariableChange {
        mode: Some(Mode::Saved),
        ..Default::default()
    };
    assert!(matches!(
        vars::change(&db, daemon, "s1", "API_TOKEN", &to_saved),
        Err(ContextError::SecretStays(_))
    ));

    // Saved to secret: the value leaves SQLite, its WAL and the screen.
    let to_secret = VariableChange {
        mode: Some(Mode::Secret),
        ..Default::default()
    };
    let t = vars::change(&db, daemon, "s1", "ticket", &to_secret).unwrap();
    assert_eq!((t.mode, t.value), (Mode::Secret, None));
    assert!(
        !on_disk(&dbdir, "Add audit export endpoint"),
        "the old value stays on disk"
    );
    assert!(!on_disk(&dbdir, "tok-example-7f3a"));
    assert!(!on_disk(&dbdir, "2026-10-01"));

    // Session to saved: the value lands in SQLite and leaves the daemon.
    let f = vars::change(&db, daemon, "s1", "from_date", &to_saved).unwrap();
    assert_eq!(
        (f.mode, f.value.as_deref()),
        (Mode::Saved, Some("2026-10-01"))
    );
    assert_eq!(
        vars::list(&db, daemon, "s1")
            .unwrap()
            .iter()
            .filter(|v| v.name == "from_date")
            .count(),
        1
    );

    vars::remove(&db, daemon, "s1", "API_TOKEN").unwrap();
    vars::remove(&db, daemon, "s1", "from_date").unwrap();
    let left: Vec<String> = vars::list(&db, daemon, "s1")
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(left, ["ticket"]);
}
