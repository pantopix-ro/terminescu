//! The core's runtime against the real daemon (task 2.6).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use terminescu_core::daemon::DaemonClient;
use terminescu_core::launcher::ensure_running;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, Core, Patch, PatchBatch, Workspace};
use terminescu_protocol::PaneInfo;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    dir: PathBuf,
    socket: PathBuf,
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
        Self {
            socket: dir.join("ptyd.sock"),
            dir,
            started: Mutex::default(),
        }
    }

    /// A runtime whose daemon exits 3 s after its last client, so tests leave nothing behind.
    fn start(&self) -> (Runtime, String, Arc<Mutex<Vec<PatchBatch>>>) {
        let batches: Arc<Mutex<Vec<PatchBatch>>> = Arc::default();
        let b = batches.clone();
        let (socket, log) = (self.socket.clone(), self.dir.join("ptyd.log"));
        let (rt, window) = Runtime::start(
            Core::new(Workspace::default()),
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
        )
        .unwrap();
        self.started.lock().unwrap().push(rt.clone());
        (rt, window, batches)
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

#[test]
fn tabs_own_panes_from_spawn_to_close() {
    let f = Fixture::new();
    let (rt, window, _) = f.start();
    let ws = rt.workspace();
    let [(side1, tab1)] = tabs(&ws).try_into().unwrap();
    let pane1 = rt.pane_of(&tab1).expect("first tab has a pane");
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
    let pane2 = rt.pane_of(&tab2).expect("new tab has a pane");
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
    assert_eq!(rt.pane_of(&tab2), None);
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
    let pane = rt.pane_of(&tab).unwrap();
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
fn a_new_core_adopts_live_panes_instead_of_starting_a_shell() {
    let f = Fixture::new();
    let (first, _, _) = f.start();
    let [(side, _)] = tabs(&first.workspace()).try_into().unwrap();
    first
        .command(Command::CreateTab { side_tab_id: side })
        .unwrap();
    let mut before: Vec<u32> = tabs(&first.workspace())
        .iter()
        .map(|(_, t)| first.pane_of(t).unwrap())
        .collect();

    // A restarted app: a fresh core on the same daemon.
    let (second, _, _) = f.start();
    let mut after: Vec<u32> = tabs(&second.workspace())
        .iter()
        .map(|(_, t)| second.pane_of(t).unwrap())
        .collect();
    before.sort();
    after.sort();
    assert_eq!(before, after);
    assert_eq!(panes(&second).len(), 2, "no extra shell");
}

#[test]
fn a_dropped_connection_reconnects_to_the_same_panes() {
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let pane = rt.pane_of(&tab).unwrap();
    let old = rt.daemon().unwrap();
    old.shutdown();
    wait_for(
        || rt.daemon().is_some_and(|d| !Arc::ptr_eq(&d, &old)),
        "reconnect",
    );
    assert_eq!(rt.pane_of(&tab), Some(pane));
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
    let pane = rt.pane_of(&tab).expect("the tab got a new pane");
    assert!(live(&rt, pane).is_some());
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
    let win = Window::default();
    let view = rt.open_view(&tab, 100, 30, win.sink()).unwrap();
    wait_for(|| win.count() > 0, "the snapshot");
    assert!(
        win.batches.lock().unwrap()[0].starts_with(b"\x1bc"),
        "a view starts from a reset screen"
    );
    let pane = rt.pane_of(&tab).unwrap();
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
    let view = rt.open_view(&tab, 80, 24, win.sink()).unwrap();
    rt.view_input(view, b"echo new:$((40+3))\r".to_vec());
    wait_for(|| win.text().contains("new:43"), "output of the new tab");
    assert!(rt.open_view("no-such-tab", 80, 24, win.sink()).is_err());
}

#[test]
fn a_view_that_falls_behind_resyncs_from_a_snapshot() {
    use terminescu_core::views::{ACK_WINDOW, ViewState};
    let f = Fixture::new();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let win = Window::default();
    let view = rt.open_view(&tab, 80, 24, win.sink()).unwrap();
    wait_for(|| win.count() > 0, "the snapshot");
    rt.view_ack(view, win.bytes());

    // About 4.4 MB with no acknowledgement past the snapshot.
    rt.view_input(
        view,
        b"yes 0123456789 | head -n 400000; echo done:$((40+4))\r".to_vec(),
    );
    wait_for(|| rt.view_state(&tab) == Some(ViewState::Stale), "stale");
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
    assert_eq!(rt.view_state(&tab), Some(ViewState::Live));
}

/// D35: secrets live in the daemon's memory only. The app's database, written the way the app
/// writes it, holds no trace of a secret set and read through the real runtime and daemon.
#[test]
fn the_app_database_never_holds_a_secret() {
    use terminescu_core::db::Db;
    use terminescu_protocol::client::Client;
    use terminescu_protocol::{Control, Frame, VarMode};

    const SECRET: &str = "tok-example-7f3c9a";
    let f = Fixture::new();
    let db_path = f.dir.join("terminescu.sqlite");
    let db = Db::open(&db_path).unwrap();
    db.local_owner().unwrap();
    let (rt, _, _) = f.start();
    let [(_, tab)] = tabs(&rt.workspace()).try_into().unwrap();
    let pane = rt.pane_of(&tab).unwrap();
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
    drop(db);
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
