//! The core's runtime against the real daemon (tasks 2.6, 3.1, 3.3, 3.4).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::launcher::ensure_running;
use terminescu_core::layout::{Dir, Layout};
use terminescu_core::persist;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, Core, Patch, PatchBatch, Workspace};
use terminescu_protocol::PaneInfo;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    dir: PathBuf,
    socket: PathBuf,
    /// The app's database, written through `Persist` the way the app writes it.
    db: Arc<Mutex<Db>>,
    started: Mutex<Vec<Runtime>>,
}

impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "tdc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(dir.join("terminescu.sqlite")).unwrap();
        db.local_owner().unwrap();
        Self {
            socket: dir.join("ptyd.sock"),
            dir,
            db: Arc::new(Mutex::new(db)),
            started: Mutex::default(),
        }
    }

    /// A runtime whose daemon exits 3 s after its last client, so tests leave nothing behind.
    /// It starts from whatever an earlier runtime of this fixture saved, like the app does.
    fn start(&self) -> (Runtime, String, Arc<Mutex<Vec<PatchBatch>>>) {
        let batches: Arc<Mutex<Vec<PatchBatch>>> = Arc::default();
        let b = batches.clone();
        let (socket, log) = (self.socket.clone(), self.dir.join("ptyd.log"));
        let saved = persist::load(&self.db.lock().unwrap()).unwrap();
        let db = self.db.clone();
        let (rt, windows) = Runtime::start(
            Core::new(Workspace::default()),
            saved,
            Box::new(move |batch| b.lock().unwrap().push(batch.clone())),
            Box::new(move |events| {
                ensure_running(
                    env!("CARGO_BIN_EXE_terminescu-ptyd").as_ref(),
                    &socket,
                    &log,
                    &["--idle-exit-secs", "3"],
                    Duration::from_secs(10),
                )?;
                DaemonClient::connect(&socket, events)
            }),
            Box::new(move |saved| persist::save(&db.lock().unwrap(), saved).unwrap()),
        )
        .unwrap();
        self.started.lock().unwrap().push(rt.clone());
        (rt, windows[0].clone(), batches)
    }

    /// Ends the daemon, the way a reboot does.
    fn kill_daemon(&self) {
        let _ = std::process::Command::new("pkill")
            .args(["-9", "-f", self.socket.to_str().unwrap()])
            .status();
        wait_for(
            || {
                !std::process::Command::new("pgrep")
                    .args(["-f", self.socket.to_str().unwrap()])
                    .status()
                    .unwrap()
                    .success()
            },
            "the daemon to end",
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Stop the runtimes first, or they would start a new daemon when this one goes.
        for rt in self.started.lock().unwrap().iter() {
            rt.stop();
        }
        // The shells keep the detached daemon alive; end it with the test.
        let _ = std::process::Command::new("pkill")
            .args(["-f", self.socket.to_str().unwrap()])
            .status();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn wait_for(mut ok: impl FnMut() -> bool, what: &str) {
    let start = Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn panes(rt: &Runtime) -> Vec<PaneInfo> {
    rt.daemon()
        .map(|d| d.list().unwrap_or_default())
        .unwrap_or_default()
}

fn live(rt: &Runtime, pane: u32) -> Option<PaneInfo> {
    panes(rt)
        .into_iter()
        .find(|p| p.pane == pane && p.exited.is_none())
}

fn tabs(ws: &Workspace) -> Vec<(String, String)> {
    ws.windows
        .iter()
        .flat_map(|w| &w.side_tabs)
        .flat_map(|s| s.tabs.iter().map(|t| (s.id.clone(), t.id.clone())))
        .collect()
}

/// The first pane of a tab.
fn pane(ws: &Workspace, tab: &str) -> String {
    ws.tab(tab).unwrap().layout.panes()[0].clone()
}

/// Every pane in reading order.
fn all_panes(ws: &Workspace) -> Vec<String> {
    ws.pane_sides().into_iter().map(|(p, _)| p).collect()
}

#[test]
fn tabs_own_panes_from_spawn_to_close() {
    let f = Fixture::new();
    let (rt, window, _) = f.start();
    let ws = rt.workspace();
    let [(side1, tab1)] = tabs(&ws).try_into().unwrap();
    let pane1 = rt
        .daemon_of(&pane(&ws, &tab1))
        .expect("first tab has a pane");
    assert_eq!(
        live(&rt, pane1).unwrap().side_tab.as_deref(),
        Some(side1.as_str())
    );

    rt.command(Command::CreateSideTab {
        window_id: window,
        index: None,
    })
    .unwrap();
    let (side2, tab2) = tabs(&rt.workspace())[1].clone();
    let p2 = pane(&rt.workspace(), &tab2);
    let pane2 = rt.daemon_of(&p2).expect("new tab has a pane");
    assert_ne!(pane1, pane2);
    assert_eq!(
        live(&rt, pane2).unwrap().side_tab.as_deref(),
        Some(side2.as_str())
    );

    // The daemon learns the new side tab when the tab moves (D36).
    rt.command(Command::MoveTab {
        tab_id: tab2.clone(),
        to_side_tab_id: side1.clone(),
        index: 1,
    })
    .unwrap();
    wait_for(
        || live(&rt, pane2).and_then(|p| p.side_tab).as_deref() == Some(side1.as_str()),
        "side tab update",
    );

    rt.command(Command::CloseTab {
        tab_id: tab2.clone(),
    })
    .unwrap();
    assert_eq!(rt.daemon_of(&p2), None);
    wait_for(
        || panes(&rt).iter().all(|p| p.pane != pane2),
        "pane removal",
    );
    assert!(live(&rt, pane1).is_some());
}

#[test]
fn a_shell_that_exits_closes_its_tab() {
    let f = Fixture::new();
    let (rt, _, batches) = f.start();
    let [(side, _)] = tabs(&rt.workspace()).try_into().unwrap();
    rt.command(Command::CreateTab { side_tab_id: side })
        .unwrap();
    let (_, tab) = tabs(&rt.workspace())[1].clone();
    let pane = rt.daemon_of(&pane(&rt.workspace(), &tab)).unwrap();
    rt.daemon()
        .unwrap()
        .input(pane, b"exit\n".to_vec())
        .unwrap();
    wait_for(|| tabs(&rt.workspace()).len() == 1, "tab closed after exit");
    assert!(
        batches
            .lock()
            .unwrap()
            .iter()
            .any(|b| b.patches.contains(&Patch::TabClosed {
                tab_id: tab.clone()
            }))
    );
    wait_for(|| panes(&rt).iter().all(|p| p.pane != pane), "pane removal");
}

#[test]
fn a_new_core_without_a_saved_layout_adopts_live_panes() {
    let f = Fixture::new();
    let (first, _, _) = f.start();
    let [(side, _)] = tabs(&first.workspace()).try_into().unwrap();
    first
        .command(Command::CreateTab { side_tab_id: side })
        .unwrap();
    let ws = first.workspace();
    let mut before: Vec<u32> = all_panes(&ws)
        .iter()
        .map(|p| first.daemon_of(p).unwrap())
        .collect();
    first.stop();

    // A restarted app that lost its database: a fresh core on the same daemon.
    f.db.lock()
        .unwrap()
        .conn()
        .execute_batch("DELETE FROM windows")
        .unwrap();
    let (second, _, _) = f.start();
    let mut after: Vec<u32> = all_panes(&second.workspace())
        .iter()
        .map(|p| second.daemon_of(p).unwrap())
        .collect();
    before.sort();
    after.sort();
    assert_eq!(before, after);
    assert_eq!(panes(&second).len(), 2, "no extra shell");
}

/// Task 3.3: quit with side tabs and splits, start, every pane shows its live shell.
#[test]
fn a_restart_restores_the_layout_and_reattaches_every_pane() {
    let f = Fixture::new();
    let (first, window, _) = f.start();
    first
        .command(Command::CreateSideTab {
            window_id: window.clone(),
            index: None,
        })
        .unwrap();
    first
        .command(Command::CreateSideTab {
            window_id: window,
            index: None,
        })
        .unwrap();
    let p0 = all_panes(&first.workspace())[0].clone();
    first
        .command(Command::SplitPane {
            pane_id: p0.clone(),
            dir: Dir::Row,
        })
        .unwrap();
    let p1 = all_panes(&first.workspace())[1].clone();
    first
        .command(Command::SplitPane {
            pane_id: p1,
            dir: Dir::Column,
        })
        .unwrap();
    let ws = first.workspace();
    assert_eq!(ws.windows[0].side_tabs.len(), 3);
    let daemons: Vec<Option<u32>> = all_panes(&ws).iter().map(|p| first.daemon_of(p)).collect();
    assert_eq!(daemons.len(), 5);
    assert!(daemons.iter().all(Option::is_some));
    first.stop();

    let (second, _, _) = f.start();
    assert_eq!(
        second.workspace(),
        ws,
        "same windows, side tabs, tabs and splits"
    );
    let after: Vec<Option<u32>> = all_panes(&ws).iter().map(|p| second.daemon_of(p)).collect();
    assert_eq!(after, daemons, "every pane on its old shell");
    assert_eq!(panes(&second).len(), 5, "no extra shell");

    // The reattached pane shows its live shell.
    let win = Window::default();
    let view = second.open_view(&p0, 80, 24, win.sink()).unwrap();
    second.view_input(view, b"echo back:$((40+5))\r".to_vec());
    wait_for(
        || win.text().contains("back:45"),
        "output after the restart",
    );
}

/// Task 3.4: after a reboot every pane gets a fresh shell in its last cwd, and a pane that ran a
/// Claude session offers to resume it. Nothing reaches the pane until the person presses it.
#[test]
fn a_reboot_restores_the_layout_with_fresh_shells_and_resume_offers() {
    let f = Fixture::new();
    let (first, _, _) = f.start();
    let p0 = all_panes(&first.workspace())[0].clone();
    first
        .command(Command::SplitPane {
            pane_id: p0.clone(),
            dir: Dir::Row,
        })
        .unwrap();
    let p1 = all_panes(&first.workspace())[1].clone();
    let work = f.dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let work = work.canonicalize().unwrap();
    let d0 = first.daemon_of(&p0).unwrap();
    first
        .daemon()
        .unwrap()
        .input(d0, format!("cd {}\r", work.display()).into_bytes())
        .unwrap();
    wait_for(
        || {
            first.refresh_cwds();
            live(&first, d0).and_then(|p| p.shell.cwd) == Some(work.display().to_string())
        },
        "the cwd report",
    );
    first.hook(
        d0,
        &serde_json::json!({"hook_event_name": "SessionStart", "session_id": "sess-0b5c"}),
    );
    // A session that ended offers nothing.
    let d1 = first.daemon_of(&p1).unwrap();
    first.hook(
        d1,
        &serde_json::json!({"hook_event_name": "SessionStart", "session_id": "sess-1"}),
    );
    first.hook(
        d1,
        &serde_json::json!({"hook_event_name": "SessionEnd", "session_id": "sess-1"}),
    );
    first.stop();
    f.kill_daemon();

    let (second, _, _) = f.start();
    let ws = second.workspace();
    assert_eq!(all_panes(&ws), [p0.clone(), p1.clone()]);
    let layout = &ws.windows[0].side_tabs[0].tabs[0].layout;
    assert_eq!(layout.resume_of(&p0), Some("sess-0b5c"));
    assert_eq!(layout.resume_of(&p1), None);
    let n0 = second.daemon_of(&p0).expect("a fresh shell");
    wait_for(
        || live(&second, n0).and_then(|p| p.shell.cwd) == Some(work.display().to_string()),
        "the fresh shell in the last cwd",
    );

    // `cat` takes whatever reaches the pane, so the test sees the typed line without running it.
    let typed = f.dir.join("typed.txt");
    let win = Window::default();
    let view = second.open_view(&p0, 80, 24, win.sink()).unwrap();
    second.view_input(
        view,
        format!("echo ready; cat > {}\r", typed.display()).into_bytes(),
    );
    wait_for(|| win.text().contains("ready\r\n"), "cat to run");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        std::fs::read_to_string(&typed).unwrap(),
        "",
        "nothing typed before the press"
    );
    second
        .command(Command::ResumePane {
            pane_id: p0.clone(),
        })
        .unwrap();
    wait_for(
        || std::fs::read_to_string(&typed).unwrap_or_default() == "claude --resume sess-0b5c\n",
        "the resume line",
    );
    let layout = second
        .workspace()
        .tab(&tabs(&ws)[0].1)
        .unwrap()
        .layout
        .clone();
    assert_eq!(
        layout.resume_of(&p0),
        None,
        "the offer is gone once pressed"
    );
    assert!(matches!(layout, Layout::Split { .. }));
}

#[test]
fn a_dropped_connection_reconnects_to_the_same_panes() {
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let p = pane(&rt.workspace(), &tab);
    let pane = rt.daemon_of(&p).unwrap();
    let old = rt.daemon().unwrap();
    old.shutdown();
    wait_for(
        || rt.daemon().is_some_and(|d| !Arc::ptr_eq(&d, &old)),
        "reconnect",
    );
    assert_eq!(rt.daemon_of(&p), Some(pane));
    assert!(live(&rt, pane).is_some());
}

#[test]
fn tabs_get_fresh_shells_when_the_daemon_dies() {
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let old = rt.daemon().unwrap();
    let pid = std::process::Command::new("pgrep")
        .args(["-f", f.socket.to_str().unwrap()])
        .output()
        .unwrap();
    let pid = String::from_utf8(pid.stdout).unwrap();
    let pid = pid.lines().next().expect("daemon pid").trim().to_owned();
    assert!(
        std::process::Command::new("kill")
            .args(["-9", &pid])
            .status()
            .unwrap()
            .success()
    );

    wait_for(
        || rt.daemon().is_some_and(|d| d.instance != old.instance),
        "a new daemon",
    );
    let pane = rt
        .daemon_of(&pane(&rt.workspace(), &tab))
        .expect("the tab got a new pane");
    assert!(live(&rt, pane).is_some());
}

fn pane_of_tab(rt: &Runtime, tab: &str) -> String {
    pane(&rt.workspace(), tab)
}

/// A window stand-in: collects every batch, and acknowledges only when told to.
#[derive(Clone, Default)]
struct Window {
    batches: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Window {
    fn sink(&self) -> terminescu_core::views::Sink {
        let b = self.batches.clone();
        Arc::new(move |batch| {
            b.lock().unwrap().push(batch);
            true
        })
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.batches.lock().unwrap().concat()).into_owned()
    }
    fn bytes(&self) -> u64 {
        self.batches
            .lock()
            .unwrap()
            .iter()
            .map(|b| b.len() as u64)
            .sum()
    }
    fn count(&self) -> usize {
        self.batches.lock().unwrap().len()
    }
}

#[test]
fn a_view_shows_the_snapshot_then_live_output_and_takes_input() {
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let p = pane(&rt.workspace(), &tab);
    let win = Window::default();
    let view = rt.open_view(&p, 100, 30, win.sink()).unwrap();
    wait_for(|| win.count() > 0, "the snapshot");
    assert!(
        win.batches.lock().unwrap()[0].starts_with(b"\x1bc"),
        "a view starts from a reset screen"
    );
    let pane = rt.daemon_of(&p).unwrap();
    assert_eq!(
        live(&rt, pane).map(|p| (p.cols, p.rows)),
        Some((100, 30)),
        "the pane takes the view's size"
    );

    rt.view_input(view, b"echo view:$((40+2))\r".to_vec());
    wait_for(|| win.text().contains("view:42"), "echoed output");
    rt.view_resize(view, 120, 40);
    wait_for(
        || live(&rt, pane).map(|p| (p.cols, p.rows)) == Some((120, 40)),
        "resize",
    );

    rt.close_view(view);
    let before = win.count();
    rt.view_input(view, b"echo gone\r".to_vec());
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(win.count(), before, "a closed view gets nothing");
}

#[test]
fn a_view_of_a_new_tab_attaches_once_its_shell_runs() {
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(side, _)] = tabs(&rt.workspace()).try_into().unwrap();
    rt.command(Command::CreateTab { side_tab_id: side })
        .unwrap();
    let (_, tab) = tabs(&rt.workspace())[1].clone();
    let win = Window::default();
    let view = rt
        .open_view(&pane(&rt.workspace(), &tab), 80, 24, win.sink())
        .unwrap();
    rt.view_input(view, b"echo new:$((40+3))\r".to_vec());
    wait_for(|| win.text().contains("new:43"), "output of the new tab");
    assert!(rt.open_view("no-such-pane", 80, 24, win.sink()).is_err());
}

#[test]
fn a_view_that_falls_behind_resyncs_from_a_snapshot() {
    use terminescu_core::views::{ACK_WINDOW, ViewState};
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let p = pane(&rt.workspace(), &tab);
    let win = Window::default();
    let view = rt.open_view(&p, 80, 24, win.sink()).unwrap();
    wait_for(|| win.count() > 0, "the snapshot");
    rt.view_ack(view, win.bytes());

    // About 4.4 MB with no acknowledgement past the snapshot.
    rt.view_input(
        view,
        b"yes 0123456789 | head -n 400000; echo done:$((40+4))\r".to_vec(),
    );
    wait_for(|| rt.view_state(&p) == Some(ViewState::Stale), "stale");
    std::thread::sleep(Duration::from_millis(500));
    let flown = win.bytes();
    assert!(flown <= ACK_WINDOW + 64 * 1024, "{flown} bytes forwarded");

    // Now a window that keeps up.
    let n = win.count();
    let mut acked = 0;
    wait_for(
        || {
            let total = win.bytes();
            rt.view_ack(view, total - acked);
            acked = total;
            win.text().contains("done:44")
        },
        "the end of the flood",
    );
    let after = win.batches.lock().unwrap()[n..].concat();
    assert!(
        after.starts_with(b"\x1bc"),
        "the resync starts from a reset screen"
    );
    assert_eq!(rt.view_state(&p), Some(ViewState::Live));
}

/// D35: secrets live in the daemon's memory only. The app's database, written the way the app
/// writes it, holds no trace of a secret set and read through the real runtime and daemon.
#[test]
fn the_app_database_never_holds_a_secret() {
    use terminescu_protocol::client::Client;
    use terminescu_protocol::{Control, Frame, VarMode};

    const SECRET: &str = "tok-example-7f3c9a";
    let f = Fixture::new();
    let db_path = f.dir.join("terminescu.sqlite");
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let pane = rt.daemon_of(&pane(&rt.workspace(), &tab)).unwrap();
    wait_for(
        || live(&rt, pane).is_some_and(|p| p.side_tab.is_some()),
        "side tab",
    );

    let mut c = Client::connect(&f.socket, "test").unwrap();
    c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut call = |frame| {
        c.send(&Frame::Control(frame)).unwrap();
        c.recv_until(|f| match f {
            Frame::Control(a @ (Control::Vars { .. } | Control::Done { .. })) => Some(a.clone()),
            Frame::Control(Control::Error { message, .. }) => panic!("{message}"),
            _ => None,
        })
        .unwrap()
    };
    call(Control::VarSet {
        req: 1,
        pane,
        name: "API_TOKEN".into(),
        value: SECRET.into(),
        mode: VarMode::Secret,
        ttl_secs: None,
    });
    let Control::Vars { vars, .. } = call(Control::VarGet {
        req: 2,
        pane,
        name: None,
    }) else {
        panic!("no vars")
    };
    assert_eq!(vars[0].value, SECRET);
    // A command and a stop both save the layout.
    rt.command(Command::SplitPane {
        pane_id: pane_of_tab(&rt, &tab),
        dir: Dir::Row,
    })
    .unwrap();
    rt.stop();

    let mut found = Vec::new();
    for entry in std::fs::read_dir(&f.dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file()
            && std::fs::read(&path)
                .unwrap()
                .windows(SECRET.len())
                .any(|w| w == SECRET.as_bytes())
        {
            found.push(path);
        }
    }
    assert!(db_path.exists());
    assert!(found.is_empty(), "secret written to {found:?}");
}
