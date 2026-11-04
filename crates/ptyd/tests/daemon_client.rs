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
