//! The core with its shells: workspace state plus the daemon panes behind each tab (task 2.6).
//!
//! Every command runs under the core lock and emits its patch batch there, so windows see batches
//! in rev order. The daemon work a batch implies (spawn, close, side tab changes) runs after the
//! core lock is released but under the effects lock, which the command takes before letting go
//! of the core lock, so effects also run in rev order.

use crate::daemon::{DaemonClient, DaemonError, DaemonEvent};
use crate::state::{Command, CommandError, Core, Id, Patch, PatchBatch, Snapshot, Workspace};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use terminescu_protocol::{PaneId, SpawnSpec};

/// Size of a new pane until its view reports its own (task 2.5).
const DEFAULT_SIZE: (u16, u16) = (80, 24);
const RECONNECT_DELAY: Duration = Duration::from_millis(500);

pub type Emit = Box<dyn Fn(&PatchBatch) + Send + Sync>;
/// Makes sure a daemon runs, then connects to it. Called at start and after a drop.
pub type Connect =
    Box<dyn Fn(mpsc::Sender<DaemonEvent>) -> Result<DaemonClient, DaemonError> + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

enum Effect {
    Spawn { tab: Id, side_tab: Id },
    Close { tab: Id },
    SideTab { tab: Id, side_tab: Id },
}

#[derive(Default)]
struct Panes {
    by_tab: HashMap<Id, PaneId>,
    by_pane: HashMap<PaneId, Id>,
}

impl Panes {
    fn link(&mut self, tab: Id, pane: PaneId) {
        self.by_pane.insert(pane, tab.clone());
        self.by_tab.insert(tab, pane);
    }
    fn unlink_tab(&mut self, tab: &str) -> Option<PaneId> {
        let pane = self.by_tab.remove(tab)?;
        self.by_pane.remove(&pane);
        Some(pane)
    }
}

struct Inner {
    core: Mutex<Core>,
    effects: Mutex<()>,
    daemon: Mutex<Option<Arc<DaemonClient>>>,
    instance: Mutex<String>,
    panes: Mutex<Panes>,
    emit: Emit,
    connect: Connect,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

impl Runtime {
    /// Connects to the daemon and builds the first window. The daemon's live panes become the
    /// tabs of that window; with none, the window gets one new login shell. Returns the window id.
    pub fn start(core: Core, emit: Emit, connect: Connect) -> Result<(Self, Id), DaemonError> {
        let (tx, rx) = mpsc::channel();
        let client = Arc::new(connect(tx)?);
        let live: Vec<_> = client
            .list()?
            .into_iter()
            .filter(|p| p.exited.is_none())
            .collect();
        let rt = Self {
            inner: Arc::new(Inner {
                core: Mutex::new(core),
                effects: Mutex::new(()),
                daemon: Mutex::new(Some(client.clone())),
                instance: Mutex::new(client.instance.clone()),
                panes: Mutex::new(Panes::default()),
                emit,
                connect,
                stopped: AtomicBool::new(false),
            }),
        };
        let (window, side_tab, tabs) = {
            let mut core = lock(&rt.inner.core);
            let (window, tabs) = core.first_start_with(live.len());
            let side_tab = core
                .workspace()
                .locate_tab(&tabs[0])
                .map(|(s, _)| s.id.clone())
                .unwrap_or_default();
            (window, side_tab, tabs)
        };
        if live.is_empty() {
            rt.spawn_for(&client, &tabs[0], &side_tab);
        } else {
            let mut panes = lock(&rt.inner.panes);
            for (tab, info) in tabs.iter().zip(&live) {
                panes.link(tab.clone(), info.pane);
                let _ = client.set_side_tab(info.pane, &side_tab);
            }
        }
        let me = rt.clone();
        std::thread::Builder::new()
            .name("daemon-events".into())
            .spawn(move || me.events(rx))
            .map_err(DaemonError::Io)?;
        Ok((rt, window))
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.inner.core).snapshot()
    }

    pub fn workspace(&self) -> Workspace {
        lock(&self.inner.core).workspace().clone()
    }

    /// The daemon pane behind a tab.
    pub fn pane_of(&self, tab: &str) -> Option<PaneId> {
        lock(&self.inner.panes).by_tab.get(tab).copied()
    }

    pub fn daemon(&self) -> Option<Arc<DaemonClient>> {
        lock(&self.inner.daemon).clone()
    }

    /// Disconnects for good, as the app quits. The shells keep running in the daemon.
    pub fn stop(&self) {
        self.inner.stopped.store(true, Ordering::SeqCst);
        if let Some(client) = lock(&self.inner.daemon).take() {
            client.shutdown();
        }
    }

    pub fn command(&self, cmd: Command) -> Result<u64, CommandError> {
        let mut core = lock(&self.inner.core);
        let batch = core.handle(cmd)?;
        (self.inner.emit)(&batch);
        let effects = plan_effects(core.workspace(), &batch);
        let guard = lock(&self.inner.effects);
        drop(core);
        self.run(effects);
        drop(guard);
        Ok(batch.rev)
    }

    fn run(&self, effects: Vec<Effect>) {
        let Some(client) = self.daemon() else {
            // Offline: closed tabs just lose their link; the reconnect pass spawns for new tabs.
            let mut panes = lock(&self.inner.panes);
            for e in effects {
                if let Effect::Close { tab } = e {
                    panes.unlink_tab(&tab);
                }
            }
            return;
        };
        for e in effects {
            match e {
                Effect::Spawn { tab, side_tab } => self.spawn_for(&client, &tab, &side_tab),
                Effect::Close { tab } => {
                    if let Some(pane) = lock(&self.inner.panes).unlink_tab(&tab) {
                        let _ = client.close(pane);
                    }
                }
                Effect::SideTab { tab, side_tab } => {
                    if let Some(pane) = self.pane_of(&tab) {
                        let _ = client.set_side_tab(pane, &side_tab);
                    }
                }
            }
        }
    }

    fn spawn_for(&self, client: &DaemonClient, tab: &str, side_tab: &str) {
        let spec = SpawnSpec {
            cols: DEFAULT_SIZE.0,
            rows: DEFAULT_SIZE.1,
            side_tab: Some(side_tab.into()),
            ..Default::default()
        };
        match client.spawn(spec) {
            Ok(pane) => lock(&self.inner.panes).link(tab.into(), pane),
            Err(e) => eprintln!("terminescu: could not start a shell for tab {tab}: {e}"),
        }
    }

    fn events(self, mut rx: Receiver<DaemonEvent>) {
        loop {
            while let Ok(event) = rx.recv() {
                match event {
                    DaemonEvent::Exited { pane, .. } => self.pane_exited(pane),
                    DaemonEvent::Disconnected => break,
                    // Output and attach go to views once task 2.5 subscribes them.
                    DaemonEvent::Output { .. } | DaemonEvent::Attached { .. } => {}
                }
            }
            *lock(&self.inner.daemon) = None;
            if self.inner.stopped.load(Ordering::SeqCst) {
                return;
            }
            rx = self.reconnect();
        }
    }

    /// A shell that exits closes its tab.
    fn pane_exited(&self, pane: PaneId) {
        let tab = lock(&self.inner.panes).by_pane.get(&pane).cloned();
        match tab {
            Some(tab) => {
                if let Err(e) = self.command(Command::CloseTab { tab_id: tab }) {
                    eprintln!("terminescu: closing the tab of pane {pane}: {e:?}");
                }
            }
            None => {
                if let Some(c) = self.daemon() {
                    let _ = c.close(pane);
                }
            }
        }
    }

    /// Reconnects until it works. Tabs whose pane the daemon no longer has get a fresh shell.
    fn reconnect(&self) -> Receiver<DaemonEvent> {
        loop {
            let (tx, rx) = mpsc::channel();
            if self.inner.stopped.load(Ordering::SeqCst) {
                return rx;
            }
            match (self.inner.connect)(tx) {
                Ok(client) => {
                    let client = Arc::new(client);
                    // A new daemon reuses pane ids: none of ours mean anything there.
                    if lock(&self.inner.instance).as_str() != client.instance {
                        let mut panes = lock(&self.inner.panes);
                        panes.by_tab.clear();
                        panes.by_pane.clear();
                    }
                    let live: Vec<PaneId> = match client.list() {
                        Ok(l) => l
                            .into_iter()
                            .filter(|p| p.exited.is_none())
                            .map(|p| p.pane)
                            .collect(),
                        Err(_) => continue,
                    };
                    // Same lock order as `command`: core, then effects.
                    let core = lock(&self.inner.core);
                    let ws = core.workspace().clone();
                    let _guard = lock(&self.inner.effects);
                    drop(core);
                    for side in ws.windows.iter().flat_map(|w| &w.side_tabs) {
                        for tab in &side.tabs {
                            let pane = self.pane_of(&tab.id);
                            if pane.is_some_and(|p| live.contains(&p)) {
                                continue;
                            }
                            lock(&self.inner.panes).unlink_tab(&tab.id);
                            self.spawn_for(&client, &tab.id, &side.id);
                        }
                    }
                    lock(&self.inner.instance).clone_from(&client.instance);
                    *lock(&self.inner.daemon) = Some(client);
                    return rx;
                }
                Err(e) => {
                    eprintln!("terminescu: daemon reconnect failed: {e}");
                    std::thread::sleep(RECONNECT_DELAY);
                }
            }
        }
    }
}

/// The daemon work a batch implies, read against the workspace after the batch.
fn plan_effects(ws: &Workspace, batch: &PatchBatch) -> Vec<Effect> {
    let side_of = |tab: &str| ws.locate_tab(tab).map(|(s, _)| s.id.clone());
    let mut out = Vec::new();
    for p in &batch.patches {
        match p {
            Patch::SideTabCreated { side_tab, .. } => {
                for t in &side_tab.tabs {
                    out.push(Effect::Spawn {
                        tab: t.id.clone(),
                        side_tab: side_tab.id.clone(),
                    });
                }
            }
            Patch::TabCreated {
                side_tab_id, tab, ..
            } => {
                out.push(Effect::Spawn {
                    tab: tab.id.clone(),
                    side_tab: side_tab_id.clone(),
                });
            }
            Patch::TabClosed { tab_id } => out.push(Effect::Close {
                tab: tab_id.clone(),
            }),
            Patch::TabMoved { tab_id, .. } => {
                if let Some(side_tab) = side_of(tab_id) {
                    out.push(Effect::SideTab {
                        tab: tab_id.clone(),
                        side_tab,
                    });
                }
            }
            Patch::SideTabClosed { .. } | Patch::TabRenamed { .. } => {}
        }
    }
    out
}
