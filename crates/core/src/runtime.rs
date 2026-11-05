//! The core with its shells: workspace state plus the daemon pane behind each pane (task 2.6).
//!
//! Every command runs under the core lock and emits its patch batch there, so windows see batches
//! in rev order. The daemon work a batch implies (spawn, close, side tab changes) runs after the
//! core lock is released but under the effects lock, which the command takes before letting go
//! of the core lock, so effects also run in rev order. Lock order: core, then effects, then the
//! small locks (panes, views, daemon). `shells` comes before `status`, so shell edges reach the
//! status engine in the order they were stored.
//!
//! The daemon work comes from comparing the panes before and after a batch: a pane that appeared
//! gets a shell, a pane that left loses it, a pane that changed side tab tells the daemon.
//!
//! Views (task 2.5, D33) carry each pane's pty output to the window showing it. The daemon event
//! thread queues output into [`Views`]; a flush thread sends one batch per view per frame.
//!
//! The layout reaches SQLite through [`Persist`] after every batch and whenever a pane's cwd or
//! Claude session changes (task 3.3). A start restores it: panes whose daemon pane still runs
//! reattach, the others get a fresh login shell in their last cwd, and a pane that ran a Claude
//! session offers to resume it (D23).

use crate::agent::{self, Agent, AgentState, AgentUpdate};
use crate::daemon::{DaemonClient, DaemonError, DaemonEvent};
use crate::layout::Layout;
use crate::persist::{PaneRecord, Saved};
use crate::state::{Command, CommandError, Core, Id, PatchBatch, Snapshot, Workspace};
use crate::status::{self, Engine, Look, PaneFacts, Rule, Shown, Until};
use crate::transcript::Tail;
use crate::views::{Action, Sink, ViewId, Views};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use terminescu_protocol::core::Expiry;
use terminescu_protocol::{PaneId, PaneInfo, ShellState, SpawnSpec};

/// Size of a new pane until its view reports its own (task 2.5).
const DEFAULT_SIZE: (u16, u16) = (80, 24);
const RECONNECT_DELAY: Duration = Duration::from_millis(500);
/// One batch per view per frame.
const FRAME: Duration = Duration::from_millis(16);
/// How often the cwds of the panes are read for the next start (D23).
const CWD_POLL: Duration = Duration::from_secs(5);
/// The poll for a daemon that does not push shell changes, so statuses lag at most this long.
const SHELL_POLL: Duration = Duration::from_secs(1);
/// How many unlinked exits `pane_exited` remembers for `spawn_for`.
const EARLY_EXITS: usize = 64;
/// How often a waiting session's transcript is read for a denial (D34).
const TAIL_POLL: Duration = Duration::from_millis(250);

pub type Emit = Box<dyn Fn(&PatchBatch) + Send + Sync>;
/// Receives each change of a pane's Claude session state (task 4.2).
pub type AgentSink = Box<dyn Fn(&AgentUpdate) + Send + Sync>;
/// Every tab's and side tab's status, sent whenever one changes (D15).
pub type StatusSink = Box<dyn Fn(&BTreeMap<Id, Shown>) + Send + Sync>;
/// Hears which side tab's daemon variables changed (D21).
pub type ContextSink = Box<dyn Fn(&str) + Send + Sync>;
/// Hears the documents whose text panes left the layout (D28).
pub type DocSink = Box<dyn Fn(&[Id]) + Send + Sync>;
/// How often expiries and Claude idle rules are checked when nothing else happens.
const STATUS_TICK: Duration = Duration::from_secs(1);
/// Makes sure a daemon runs, then connects to it. Called at start and after a drop.
pub type Connect =
    Box<dyn Fn(mpsc::Sender<DaemonEvent>) -> Result<DaemonClient, DaemonError> + Send + Sync>;
/// Writes the layout for the next start. Called under the effects lock, so saves arrive in order.
pub type Persist = Box<dyn Fn(&Saved) + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A Claude session id as hooks report it. Anything else never reaches a pane as typed text.
/// It starts with a letter or digit, so it can never read as a `claude` option.
pub fn is_session_id(s: &str) -> bool {
    s.len() <= 128
        && s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

enum Effect {
    Spawn {
        pane: Id,
        side_tab: Id,
        cwd: Option<String>,
    },
    Close {
        pane: Id,
    },
    SideTab {
        pane: Id,
        side_tab: Id,
    },
    Type {
        pane: Id,
        text: String,
    },
}

#[derive(Default)]
struct Panes {
    by_id: HashMap<Id, PaneId>,
    by_daemon: HashMap<PaneId, Id>,
    /// Last cwd and Claude session per pane, kept across daemon restarts.
    meta: HashMap<Id, PaneRecord>,
    /// Daemon panes closed while offline; the next connection to the same daemon closes them.
    orphans: Vec<PaneId>,
    /// Recent exits of daemon panes with no pane linked yet: a shell can exit before
    /// `spawn_for` links it.
    early_exits: std::collections::VecDeque<PaneId>,
}

impl Panes {
    fn link(&mut self, pane: Id, daemon: PaneId) {
        self.by_daemon.insert(daemon, pane.clone());
        self.by_id.insert(pane, daemon);
    }
    fn unlink(&mut self, pane: &str) -> Option<PaneId> {
        let daemon = self.by_id.remove(pane)?;
        self.by_daemon.remove(&daemon);
        Some(daemon)
    }
    fn clear_links(&mut self) {
        self.by_id.clear();
        self.by_daemon.clear();
        self.orphans.clear();
        self.early_exits.clear();
    }
}

#[derive(Default)]
struct StatusState {
    engine: Engine,
    /// The map last sent, so an unchanged one is not sent again.
    last: BTreeMap<Id, Shown>,
    /// Per pane with a Claude session: its state and since when, for idle rules.
    seen: HashMap<Id, (AgentState, i64)>,
}

struct Inner {
    core: Mutex<Core>,
    effects: Mutex<()>,
    daemon: Mutex<Option<Arc<DaemonClient>>>,
    instance: Mutex<String>,
    panes: Mutex<Panes>,
    views: Mutex<Views>,
    /// The Claude session per pane, with a transcript tail while it waits (D34).
    agents: Mutex<HashMap<Id, (Agent, Option<Tail>)>>,
    agent_sink: Mutex<Option<AgentSink>>,
    /// Transcript files found by session id, so a polled transcript pane walks the projects
    /// folder once.
    paths: Mutex<HashMap<String, std::path::PathBuf>>,
    /// The last shell state per pane, from daemon pushes and the cwd poll.
    shells: Mutex<HashMap<Id, ShellState>>,
    status: Mutex<StatusState>,
    /// Set when something a status depends on changed; the status thread wakes on it.
    status_dirty: Mutex<bool>,
    status_wake: Condvar,
    status_sink: Mutex<Option<StatusSink>>,
    /// Launcher side tabs and the tab of each of their commands (D24).
    launchers: Mutex<HashMap<Id, Vec<Option<Id>>>>,
    /// Panes that got a fresh shell at start, most likely after a reboot; launchers rerun there.
    respawned: Mutex<Vec<Id>>,
    context_sink: Mutex<Option<ContextSink>>,
    doc_sink: Mutex<Option<DocSink>>,
    /// Settings › Shell: new shells point `VISUAL` and `EDITOR` at `terminescu edit --wait`.
    editor: AtomicBool,
    queued: Condvar,
    emit: Emit,
    connect: Connect,
    persist: Persist,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

impl Runtime {
    /// Connects to the daemon and builds the workspace. With a saved layout, its panes reattach
    /// or restart; live daemon panes the layout doesn't know join a new side tab in the first
    /// window, so no shell gets lost. Without one, the daemon's live panes become the tabs of a
    /// first window, or it gets one new login shell. Returns the window ids.
    pub fn start(
        mut core: Core,
        saved: Option<Saved>,
        emit: Emit,
        connect: Connect,
        persist: Persist,
        editor: bool,
    ) -> Result<(Self, Vec<Id>), DaemonError> {
        let (tx, rx) = mpsc::channel();
        let client = Arc::new(connect(tx)?);
        let live: Vec<PaneInfo> = client
            .list()?
            .into_iter()
            .filter(|p| p.exited.is_none())
            .collect();
        let mut panes = Panes::default();
        let mut spawn: Vec<(Id, Id, Option<String>)> = Vec::new();
        let saved = saved.filter(|s| !s.workspace.windows.is_empty());
        match saved {
            Some(saved) => {
                let same = saved.instance.as_deref() == Some(client.instance.as_str());
                let mut ws = saved.workspace;
                for (pane, side) in ws.terminal_sides() {
                    let rec = saved.panes.get(&pane).cloned().unwrap_or_default();
                    let running = rec
                        .daemon
                        .filter(|d| same && live.iter().any(|i| i.pane == *d));
                    if let Some(d) = running {
                        panes.link(pane.clone(), d);
                        panes.meta.insert(pane, rec);
                        continue;
                    }
                    // The shell is gone, most likely a reboot: a fresh one in the last cwd.
                    let session = rec.session.clone().filter(|s| is_session_id(s));
                    if let Some(session) = session {
                        set_resume(&mut ws, &pane, Some(session));
                    }
                    spawn.push((pane.clone(), side, rec.cwd.clone()));
                    panes.meta.insert(
                        pane,
                        PaneRecord {
                            daemon: None,
                            cwd: rec.cwd,
                            session: None,
                        },
                    );
                }
                core.restore(ws);
                let orphans: Vec<&PaneInfo> = live
                    .iter()
                    .filter(|i| !panes.by_daemon.contains_key(&i.pane))
                    .collect();
                let first = core.workspace().windows[0].id.clone();
                if !orphans.is_empty()
                    && let Some(ids) = core.append_side_tab(&first, orphans.len())
                {
                    for (pane, info) in ids.into_iter().zip(orphans) {
                        panes.link(pane, info.pane);
                    }
                }
            }
            None => {
                let (_, ids) = core.first_start_with(live.len().max(1));
                if live.is_empty() {
                    let side = core
                        .workspace()
                        .pane_sides()
                        .into_iter()
                        .next()
                        .map(|(_, s)| s)
                        .unwrap_or_default();
                    spawn.push((ids[0].clone(), side, None));
                } else {
                    for (pane, info) in ids.into_iter().zip(&live) {
                        panes.link(pane, info.pane);
                    }
                }
            }
        }
        for info in &live {
            if let Some(pane) = panes.by_daemon.get(&info.pane).cloned() {
                let rec = panes.meta.entry(pane).or_default();
                if info.shell.cwd.is_some() {
                    rec.cwd.clone_from(&info.shell.cwd);
                }
            }
        }
        let windows: Vec<Id> = core
            .workspace()
            .windows
            .iter()
            .map(|w| w.id.clone())
            .collect();
        let rt = Self {
            inner: Arc::new(Inner {
                core: Mutex::new(core),
                effects: Mutex::new(()),
                daemon: Mutex::new(Some(client.clone())),
                instance: Mutex::new(client.instance.clone()),
                panes: Mutex::new(panes),
                views: Mutex::new(Views::default()),
                agents: Mutex::default(),
                agent_sink: Mutex::default(),
                paths: Mutex::default(),
                shells: Mutex::default(),
                status: Mutex::default(),
                status_dirty: Mutex::new(true),
                status_wake: Condvar::new(),
                status_sink: Mutex::default(),
                launchers: Mutex::default(),
                respawned: Mutex::new(spawn.iter().map(|(p, _, _)| p.clone()).collect()),
                context_sink: Mutex::default(),
                doc_sink: Mutex::default(),
                editor: AtomicBool::new(editor),
                queued: Condvar::new(),
                emit,
                connect,
                persist,
                stopped: AtomicBool::new(false),
            }),
        };
        {
            let core = lock(&rt.inner.core);
            let ws = core.workspace().clone();
            let _guard = lock(&rt.inner.effects);
            drop(core);
            for (pane, side) in ws.terminal_sides() {
                if let Some(d) = rt.daemon_of(&pane) {
                    let _ = client.set_side_tab(d, &side);
                }
            }
            for (pane, side, cwd) in spawn {
                rt.spawn_for(&client, &pane, &side, cwd.as_deref());
            }
            rt.save(&ws);
        }
        rt.thread("daemon-events", move |me| me.events(rx))?;
        rt.thread("view-flush", |me| me.flush())?;
        rt.thread("cwd-poll", |me| me.poll_cwds())?;
        rt.thread("agent-tail", |me| me.tail_agents())?;
        rt.thread("statuses", |me| me.run_statuses())?;
        Ok((rt, windows))
    }

    fn thread(&self, name: &str, f: impl FnOnce(Self) + Send + 'static) -> Result<(), DaemonError> {
        let me = self.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || f(me))
            .map(drop)
            .map_err(DaemonError::Io)
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.inner.core).snapshot()
    }

    pub fn workspace(&self) -> Workspace {
        lock(&self.inner.core).workspace().clone()
    }

    /// The daemon pane behind a pane.
    pub fn daemon_of(&self, pane: &str) -> Option<PaneId> {
        lock(&self.inner.panes).by_id.get(pane).copied()
    }

    pub fn daemon(&self) -> Option<Arc<DaemonClient>> {
        lock(&self.inner.daemon).clone()
    }

    /// Saves the cwds and disconnects for good, as the app quits. The shells keep running in
    /// the daemon.
    pub fn stop(&self) {
        self.refresh_cwds();
        self.inner.stopped.store(true, Ordering::SeqCst);
        self.inner.queued.notify_all();
        self.mark_status();
        if let Some(client) = lock(&self.inner.daemon).take() {
            client.shutdown();
        }
    }

    /// Opens a view of a pane at the window's size. Output then reaches `sink`, starting with a
    /// snapshot of the screen. A pane whose shell is still starting attaches once it runs.
    pub fn open_view(
        &self,
        pane: &str,
        cols: u16,
        rows: u16,
        sink: Sink,
    ) -> Result<ViewId, CommandError> {
        // Core, then effects: a pane created by a command in flight has its shell once the
        // effects lock is ours.
        let core = lock(&self.inner.core);
        if core.workspace().locate_pane(pane).is_none() {
            return Err(CommandError::UnknownPane(pane.into()));
        }
        let _guard = lock(&self.inner.effects);
        drop(core);
        let (view, action) = lock(&self.inner.views).open(pane, cols, rows, sink);
        if let Some(client) = self.daemon() {
            self.act(&client, action);
        }
        Ok(view)
    }

    pub fn view_state(&self, pane: &str) -> Option<crate::views::ViewState> {
        lock(&self.inner.views).state(pane)
    }

    /// Closes a view. A view that another window replaced closes as a no-op.
    pub fn close_view(&self, view: ViewId) {
        let pane = lock(&self.inner.views).close(view);
        if let (Some(d), Some(client)) = (pane.and_then(|p| self.daemon_of(&p)), self.daemon()) {
            let _ = client.detach(d);
        }
    }

    /// The window wrote `bytes` of this view's output.
    pub fn view_ack(&self, view: ViewId, bytes: u64) {
        let action = lock(&self.inner.views).ack(view, bytes);
        if let (Some(action), Some(client)) = (action, self.daemon()) {
            self.act(&client, action);
        }
    }

    pub fn view_resize(&self, view: ViewId, cols: u16, rows: u16) {
        let pane = lock(&self.inner.views).resize(view, cols, rows);
        if let (Some(d), Some(client)) = (pane.and_then(|p| self.daemon_of(&p)), self.daemon()) {
            let _ = client.resize(d, cols, rows);
        }
    }

    /// Keys and pastes from the window, as bytes for the pty.
    pub fn view_input(&self, view: ViewId, data: Vec<u8>) {
        let pane = lock(&self.inner.views).pane_of(view);
        if let (Some(d), Some(client)) = (pane.and_then(|p| self.daemon_of(&p)), self.daemon()) {
            let _ = client.input(d, data);
        }
    }

    /// Sends what a view needs from the daemon. Resize goes first so the snapshot fits.
    fn act(&self, client: &DaemonClient, action: Action) {
        match action {
            Action::Attach { pane, cols, rows } => {
                if let Some(d) = self.daemon_of(&pane) {
                    let _ = client.resize(d, cols, rows);
                    let _ = client.attach(d);
                }
            }
        }
    }

    /// The side tab and tab of a daemon pane, for requests from `terminescu` in that pane.
    pub fn place_of_daemon(&self, daemon: PaneId) -> Option<(Id, Id)> {
        let pane = self.pane_of_daemon(daemon)?;
        let ws = self.workspace();
        ws.locate_pane(&pane)
            .map(|(side, tab)| (side.id.clone(), tab.id.clone()))
    }

    /// The layout pane a daemon pane shows in.
    pub fn pane_of_daemon(&self, daemon: PaneId) -> Option<Id> {
        lock(&self.inner.panes).by_daemon.get(&daemon).cloned()
    }

    /// Sends one batch per view per frame, sleeping while nothing is queued.
    fn flush(self) {
        loop {
            let started = Instant::now();
            let batches = {
                let mut views = lock(&self.inner.views);
                while !views.has_queued() {
                    if self.inner.stopped.load(Ordering::SeqCst) {
                        return;
                    }
                    views = self
                        .inner
                        .queued
                        .wait_timeout(views, Duration::from_secs(1))
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
                views.take_batches()
            };
            for (view, sink, bytes) in batches {
                if !sink(bytes) {
                    self.close_view(view);
                }
            }
            if let Some(rest) = FRAME.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }

    pub fn command(&self, cmd: Command) -> Result<u64, CommandError> {
        let mut core = lock(&self.inner.core);
        let before = core.workspace().terminal_sides();
        let docs_before = core.workspace().doc_panes();
        // Typed text and the cwd a split inherits, read before the batch changes the layout.
        let (typed, inherit) = match &cmd {
            Command::ResumePane { pane_id } => {
                let session = core
                    .workspace()
                    .locate_pane(pane_id)
                    .and_then(|(_, t)| t.layout.resume_of(pane_id))
                    .filter(|s| is_session_id(s))
                    .map(|s| (pane_id.clone(), format!("claude --resume {s}\r")));
                // An offer with a bad id is never typed; the core refuses it as nothing to resume.
                if session.is_none() {
                    return Err(CommandError::NothingToResume(pane_id.clone()));
                }
                (session, None)
            }
            Command::SplitPane { pane_id, .. } => (None, Some(pane_id.clone())),
            _ => (None, None),
        };
        let batch = core.handle(cmd)?;
        (self.inner.emit)(&batch);
        self.mark_status();
        let ws = core.workspace().clone();
        let guard = lock(&self.inner.effects);
        drop(core);
        let cwd = inherit.and_then(|p| self.live_cwd(&p));
        let mut effects = diff_effects(&before, &ws.terminal_sides(), cwd);
        if let Some((pane, text)) = typed {
            effects.push(Effect::Type { pane, text });
        }
        self.run(effects);
        let docs_after: HashSet<Id> = ws.doc_panes().into_iter().map(|(_, d, _)| d).collect();
        let gone: Vec<Id> = docs_before
            .into_iter()
            .map(|(_, d, _)| d)
            .filter(|d| !docs_after.contains(d))
            .collect();
        if !gone.is_empty()
            && let Some(sink) = lock(&self.inner.doc_sink).as_ref()
        {
            sink(&gone);
        }
        self.save(&ws);
        drop(guard);
        Ok(batch.rev)
    }

    /// Types bytes into a pane's shell as if the person typed them (commands, D24). False when
    /// the pane has no shell right now.
    pub fn type_into(&self, pane: &str, bytes: Vec<u8>) -> bool {
        match (self.daemon_of(pane), self.daemon()) {
            (Some(d), Some(client)) => client.input(d, bytes).is_ok(),
            _ => false,
        }
    }

    /// Runs a command that opens a tab or a side tab, then types `bytes` into its new pane. The
    /// shell holds them as typeahead until its prompt reads them. Returns the new pane and
    /// whether the bytes reached it.
    pub fn command_then_type(
        &self,
        cmd: Command,
        bytes: Vec<u8>,
    ) -> Result<(Option<Id>, bool), CommandError> {
        let before: std::collections::HashSet<Id> = self
            .workspace()
            .terminal_sides()
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        self.command(cmd)?;
        let pane = self
            .workspace()
            .terminal_sides()
            .into_iter()
            .map(|(p, _)| p)
            .find(|p| !before.contains(p));
        let typed = pane.as_ref().is_some_and(|p| self.type_into(p, bytes));
        Ok((pane, typed))
    }

    /// The cwd the shell integration reports now, else the last one seen.
    pub fn live_cwd(&self, pane: &str) -> Option<String> {
        let client = self.daemon();
        let d = self.daemon_of(pane);
        if let (Some(client), Some(d)) = (client, d)
            && let Ok(list) = client.list()
            && let Some(cwd) = list
                .into_iter()
                .find(|i| i.pane == d)
                .and_then(|i| i.shell.cwd)
        {
            return Some(cwd);
        }
        lock(&self.inner.panes)
            .meta
            .get(pane)
            .and_then(|r| r.cwd.clone())
    }

    fn run(&self, effects: Vec<Effect>) {
        let Some(client) = self.daemon() else {
            // Offline: closed panes lose their link and their shell waits for the reconnect to
            // close it; the reconnect pass also spawns for new panes.
            let mut panes = lock(&self.inner.panes);
            for e in effects {
                if let Effect::Close { pane } = e {
                    lock(&self.inner.views).remove_pane(&pane);
                    self.drop_agent(&pane);
                    if let Some(d) = panes.unlink(&pane) {
                        panes.orphans.push(d);
                    }
                    panes.meta.remove(&pane);
                }
            }
            return;
        };
        for e in effects {
            match e {
                Effect::Spawn {
                    pane,
                    side_tab,
                    cwd,
                } => self.spawn_for(&client, &pane, &side_tab, cwd.as_deref()),
                Effect::Close { pane } => {
                    lock(&self.inner.views).remove_pane(&pane);
                    let daemon = {
                        let mut panes = lock(&self.inner.panes);
                        panes.meta.remove(&pane);
                        panes.unlink(&pane)
                    };
                    // After the unlink: a hook resolves its pane under `panes`, so none lands
                    // for this pane once the agent is gone.
                    self.drop_agent(&pane);
                    if let Some(d) = daemon {
                        let _ = client.close(d);
                    }
                }
                Effect::SideTab { pane, side_tab } => {
                    if let Some(d) = self.daemon_of(&pane) {
                        let _ = client.set_side_tab(d, &side_tab);
                    }
                }
                Effect::Type { pane, text } => {
                    if let Some(d) = self.daemon_of(&pane) {
                        let _ = client.input(d, text.into_bytes());
                    }
                }
            }
        }
    }

    /// Starts a login shell for a pane, in `cwd` when given. A cwd that no longer exists falls
    /// back to the home folder.
    fn spawn_for(&self, client: &DaemonClient, pane: &str, side_tab: &str, cwd: Option<&str>) {
        // The shell integration exports `VISUAL` and `EDITOR` after the rc files (D9).
        let env: Vec<(String, String)> = if self.inner.editor.load(Ordering::SeqCst) {
            vec![("TERMINESCU_EDITOR".into(), "1".into())]
        } else {
            Vec::new()
        };
        let spec = |cwd: Option<&str>| SpawnSpec {
            cols: DEFAULT_SIZE.0,
            rows: DEFAULT_SIZE.1,
            side_tab: Some(side_tab.into()),
            cwd: cwd.map(Into::into),
            env: env.clone(),
            ..Default::default()
        };
        let cwd = cwd.filter(|c| std::path::Path::new(c).is_dir());
        let spawned = client.spawn(spec(cwd)).or_else(|e| {
            if cwd.is_some() {
                client.spawn(spec(None))
            } else {
                Err(e)
            }
        });
        match spawned {
            Ok(d) => {
                let exited = {
                    let mut panes = lock(&self.inner.panes);
                    panes.link(pane.into(), d);
                    let at = panes.early_exits.iter().position(|e| *e == d);
                    at.and_then(|i| panes.early_exits.remove(i)).is_some()
                };
                if exited {
                    // The caller holds the effects lock, so the close runs on its own thread.
                    let _ = self.thread("early-exit", move |me| me.pane_exited(d));
                    return;
                }
                let action = lock(&self.inner.views).reattach(pane);
                if let Some(action) = action {
                    self.act(client, action);
                }
            }
            Err(e) => eprintln!("terminescu: could not start a shell for pane {pane}: {e}"),
        }
    }

    /// Hands the layout to [`Persist`]. The caller holds the effects lock.
    fn save(&self, ws: &Workspace) {
        let panes = {
            let p = lock(&self.inner.panes);
            ws.pane_sides()
                .into_iter()
                .map(|(id, _)| {
                    let mut rec = p.meta.get(&id).cloned().unwrap_or_default();
                    rec.daemon = p.by_id.get(&id).copied();
                    (id, rec)
                })
                .collect()
        };
        let instance = Some(lock(&self.inner.instance).clone());
        (self.inner.persist)(&Saved {
            workspace: ws.clone(),
            panes,
            instance,
        });
    }

    /// Runs `f` on the pane records with the save ordering of a command; saves when `f` changed
    /// anything.
    fn update_meta(&self, f: impl FnOnce(&mut Panes) -> bool) {
        let core = lock(&self.inner.core);
        let ws = core.workspace().clone();
        let _guard = lock(&self.inner.effects);
        drop(core);
        let changed = f(&mut lock(&self.inner.panes));
        if changed {
            self.save(&ws);
        }
    }

    /// A Claude Code hook event from a pane (D34). The session it names becomes the pane's
    /// session until `SessionEnd`, so a reboot can offer to resume it.
    pub fn hook(&self, daemon: PaneId, event: &serde_json::Value) {
        {
            // `panes` stays held while the agent changes, so a closing pane can't get an
            // agent back after `drop_agent` (lock order: panes, then agents).
            let panes = lock(&self.inner.panes);
            if let Some(pane) = panes.by_daemon.get(&daemon) {
                self.hook_agent(pane, event);
            }
        }
        let name = event.get("hook_event_name").and_then(|v| v.as_str());
        let session = event
            .get("session_id")
            .and_then(|v| v.as_str())
            .filter(|s| is_session_id(s))
            .map(String::from);
        let next = match name {
            Some("SessionEnd") => None,
            _ if session.is_some() => session,
            _ => return,
        };
        self.update_meta(|panes| {
            let Some(pane) = panes.by_daemon.get(&daemon).cloned() else {
                return false;
            };
            let rec = panes.meta.entry(pane).or_default();
            if rec.session == next {
                return false;
            }
            rec.session = next;
            true
        });
    }

    /// The transcript file of a Claude session: the one its hooks named, else the session's
    /// file in Claude's projects folder (task 4.4).
    pub fn transcript_path(&self, session: &str) -> Option<std::path::PathBuf> {
        if !is_session_id(session) {
            return None;
        }
        let known = lock(&self.inner.agents)
            .values()
            .find(|(a, _)| a.session == session)
            .and_then(|(a, _)| a.transcript.clone());
        if known.is_some() {
            return known;
        }
        if let Some(p) = lock(&self.inner.paths).get(session).filter(|p| p.is_file()) {
            return Some(p.clone());
        }
        let found = crate::transcript::find_in(&crate::transcript::projects_dir(), session)?;
        lock(&self.inner.paths).insert(session.into(), found.clone());
        Some(found)
    }

    /// Where the UI gets agent changes; the current agents come from [`Runtime::agents`].
    pub fn on_agent(&self, sink: AgentSink) {
        *lock(&self.inner.agent_sink) = Some(sink);
    }

    /// Every pane's Claude session, for a window that just opened.
    pub fn agents(&self) -> Vec<AgentUpdate> {
        lock(&self.inner.agents)
            .iter()
            .map(|(pane, (a, _))| AgentUpdate {
                pane: pane.clone(),
                agent: Some(a.view()),
            })
            .collect()
    }

    fn tell(&self, updates: Vec<AgentUpdate>) {
        if updates.is_empty() {
            return;
        }
        if let Some(sink) = lock(&self.inner.agent_sink).as_ref() {
            for u in &updates {
                sink(u);
            }
        }
    }

    /// Agent updates go out while `agents` is held, so the UI sees them in the order the core
    /// made them. The sink only emits an event and never calls back into the runtime.
    fn drop_agent(&self, pane: &str) {
        let mut agents = lock(&self.inner.agents);
        if agents.remove(pane).is_some() {
            self.tell(vec![AgentUpdate {
                pane: pane.into(),
                agent: None,
            }]);
        }
        drop(agents);
        lock(&self.inner.shells).remove(pane);
        self.mark_status();
    }

    /// Applies a hook event to the pane's agent and tells the UI when it shows differently.
    fn hook_agent(&self, pane: &str, event: &serde_json::Value) {
        let mut agents = lock(&self.inner.agents);
        let update = {
            let (old, old_tail) = match agents.remove(pane) {
                Some((a, t)) => (Some(a), t),
                None => (None, None),
            };
            let shown = old.as_ref().map(Agent::view);
            let next = agent::on_hook(old, event);
            let after = next.as_ref().map(Agent::view);
            if let Some(a) = next {
                // Only a waiting session needs its transcript read; a denial fires no hook.
                let tail = match (a.state, &a.transcript) {
                    (AgentState::Waiting, Some(path)) => match old_tail {
                        Some(t) if t.path() == path => Some(t),
                        _ => Some(Tail::from_end(path)),
                    },
                    _ => None,
                };
                agents.insert(pane.into(), (a, tail));
            }
            (shown != after).then(|| AgentUpdate {
                pane: pane.into(),
                agent: after,
            })
        };
        let changed = update.is_some();
        self.tell(update.into_iter().collect());
        drop(agents);
        if changed {
            self.mark_status();
        }
    }

    /// Reads the transcripts of waiting sessions: a denied prompt or a finished turn ends
    /// Waiting (D34).
    fn tail_agents(self) {
        while !self.inner.stopped.load(Ordering::SeqCst) {
            std::thread::sleep(TAIL_POLL);
            // Read the files with no lock held, then apply what was read to agents that still
            // wait on the same file.
            let waiting: Vec<(Id, String, Tail)> = lock(&self.inner.agents)
                .iter()
                .filter_map(|(pane, (a, t))| Some((pane.clone(), a.session.clone(), t.clone()?)))
                .collect();
            let read: Vec<_> = waiting
                .into_iter()
                .filter_map(|(pane, session, mut t)| {
                    let from = t.offset();
                    let entries = t.read().ok()?;
                    Some((pane, session, from, t, entries))
                })
                .collect();
            let mut agents = lock(&self.inner.agents);
            let mut updates = Vec::new();
            for (pane, session, from, t, entries) in read {
                let Some((a, tail)) = agents.get_mut(&pane) else {
                    continue;
                };
                if a.session != session
                    || tail
                        .as_ref()
                        .is_none_or(|c| c.path() != t.path() || c.offset() != from)
                {
                    continue;
                }
                if entries.iter().any(|e| agent::on_entry(a, e)) {
                    *tail = None;
                    updates.push(AgentUpdate {
                        pane,
                        agent: Some(a.view()),
                    });
                } else {
                    *tail = Some(t);
                }
            }
            let changed = !updates.is_empty();
            self.tell(updates);
            drop(agents);
            if changed {
                self.mark_status();
            }
        }
    }

    /// Asks the status thread to compute the statuses again.
    /// The launcher side tabs and their commands' tabs; the statuses follow.
    pub fn set_launchers(&self, launchers: HashMap<Id, Vec<Option<Id>>>) {
        *lock(&self.inner.launchers) = launchers;
        self.mark_status();
    }

    /// The panes that started with a fresh shell, once: the app reruns launchers there.
    pub fn take_respawned(&self) -> Vec<Id> {
        std::mem::take(&mut *lock(&self.inner.respawned))
    }

    fn mark_status(&self) {
        *lock(&self.inner.status_dirty) = true;
        self.inner.status_wake.notify_one();
    }

    /// A pane's shell state from a daemon push or the poll. A stale poll result, with lower
    /// run counters than a push already seen, is ignored.
    fn shell_changed(&self, pane: &str, next: ShellState) {
        {
            // `shells` stays held while the engine sees the edge, so a push and a poll for the
            // same pane reach it in the order they were stored.
            let mut shells = lock(&self.inner.shells);
            let prev = shells.get(pane).cloned();
            if let Some(p) = &prev
                && (next.started < p.started || next.ended < p.ended || *p == next)
            {
                return;
            }
            shells.insert(pane.into(), next.clone());
            lock(&self.inner.status)
                .engine
                .shell(pane, prev.as_ref(), &next);
        }
        self.mark_status();
    }

    /// A pane got a new shell: its run counters start over, and statuses waiting for the old
    /// shell's next command go.
    fn shell_restarted(&self, pane: &str) {
        let mut shells = lock(&self.inner.shells);
        shells.remove(pane);
        lock(&self.inner.status).engine.shell_restarted(pane);
    }

    /// `terminescu status` from a daemon pane (D15, D36): sets or clears the dynamic status of
    /// that pane's tab.
    pub fn status_from_cli(
        &self,
        daemon: PaneId,
        label: Option<String>,
        color: Option<String>,
        expiry: Expiry,
    ) -> Result<(), String> {
        let pane = self
            .pane_of_daemon(daemon)
            .ok_or_else(|| "this pane is not in the layout".to_string())?;
        let look = label
            .map(|label| {
                let look = Look {
                    label: label.trim().into(),
                    accent: color,
                    background: None,
                };
                look.check().map(|_| look).map_err(|e| e.to_string())
            })
            .transpose()?;
        let until = match expiry {
            Expiry::Never => Until::Never,
            Expiry::After { ms } => {
                Until::At(crate::now_ms().saturating_add(i64::try_from(ms).unwrap_or(i64::MAX)))
            }
            Expiry::CommandEnd => Until::CommandEnd,
        };
        lock(&self.inner.status).engine.cli(&pane, look, until);
        self.mark_status();
        Ok(())
    }

    /// The rules from Settings › Statuses, after any change there.
    pub fn set_status_rules(&self, rules: Vec<Rule>) {
        lock(&self.inner.status).engine.set_rules(rules);
        self.mark_status();
    }

    /// Where the UI gets status changes; the current ones come from [`Runtime::statuses`].
    pub fn on_status(&self, sink: StatusSink) {
        *lock(&self.inner.status_sink) = Some(sink);
    }

    /// The opt-in editor setting; shells started from now on follow it (D27).
    pub fn set_editor(&self, on: bool) {
        self.inner.editor.store(on, Ordering::SeqCst);
    }

    /// Where the document store hears that the last pane of a document closed.
    pub fn on_docs_closed(&self, sink: DocSink) {
        *lock(&self.inner.doc_sink) = Some(sink);
    }

    pub fn on_context(&self, sink: ContextSink) {
        *lock(&self.inner.context_sink) = Some(sink);
    }

    /// Tells the UI a side tab's context changed outside a window, as from the CLI.
    pub fn context_changed(&self, side_tab: &str) {
        if let Some(sink) = lock(&self.inner.context_sink).as_ref() {
            sink(side_tab);
        }
    }

    /// Every tab's and side tab's status now.
    pub fn statuses(&self) -> BTreeMap<Id, Shown> {
        self.compute_statuses()
    }

    /// Recomputes the statuses and sends them when they changed. The send happens under the
    /// `status` lock, so windows get the maps in the order they were made.
    fn compute_statuses(&self) -> BTreeMap<Id, Shown> {
        let ws = self.workspace();
        let agents: HashMap<Id, AgentState> = lock(&self.inner.agents)
            .iter()
            .map(|(p, (a, _))| (p.clone(), a.state))
            .collect();
        let shells = lock(&self.inner.shells).clone();
        let now = crate::now_ms();
        let terminals: Vec<Id> = ws.terminal_sides().into_iter().map(|(p, _)| p).collect();
        let mut st = lock(&self.inner.status);
        let mut idle: HashMap<Id, Option<i64>> = HashMap::new();
        for pane in &terminals {
            let since = agents.get(pane).map(|state| {
                let seen = st.seen.entry(pane.clone()).or_insert((*state, now));
                if seen.0 != *state {
                    *seen = (*state, now);
                }
                seen.1
            });
            let resting = matches!(agents.get(pane), Some(AgentState::Done | AgentState::Idle));
            idle.insert(pane.clone(), since.filter(|_| resting));
        }
        st.seen.retain(|p, _| agents.contains_key(p));
        st.engine.tick(now, &idle);
        let live: HashSet<&str> = terminals.iter().map(String::as_str).collect();
        st.engine.retain_panes(&live);
        let facts: HashMap<Id, PaneFacts> = terminals
            .iter()
            .map(|p| {
                (
                    p.clone(),
                    PaneFacts {
                        agent: agents.get(p).copied(),
                        shell: shells.get(p).cloned(),
                    },
                )
            })
            .collect();
        let mut map = status::compute(&ws, &facts, &st.engine);
        status::apply_launchers(&mut map, &lock(&self.inner.launchers));
        if map != st.last {
            st.last = map.clone();
            if let Some(sink) = lock(&self.inner.status_sink).as_ref() {
                sink(&map);
            }
        }
        map
    }

    /// Recomputes statuses when something changed, and every second for expiries.
    fn run_statuses(self) {
        while !self.inner.stopped.load(Ordering::SeqCst) {
            {
                let dirty = lock(&self.inner.status_dirty);
                let (mut dirty, _) = self
                    .inner
                    .status_wake
                    .wait_timeout_while(dirty, STATUS_TICK, |d| !*d)
                    .unwrap_or_else(|e| e.into_inner());
                *dirty = false;
            }
            self.compute_statuses();
        }
    }

    /// Reads each pane's cwd every few seconds, so a reboot restarts shells where they were.
    fn poll_cwds(self) {
        while !self.inner.stopped.load(Ordering::SeqCst) {
            // A daemon without shell pushes gets polled faster, so statuses keep up.
            let pushes = self.daemon().is_none_or(|c| c.pushes_shell);
            std::thread::sleep(if pushes { CWD_POLL } else { SHELL_POLL });
            self.refresh_cwds();
        }
    }

    /// Reads every pane's cwd from the daemon now and saves the layout when one changed.
    pub fn refresh_cwds(&self) {
        let Some(client) = self.daemon() else {
            return;
        };
        let Ok(list) = client.list() else {
            return;
        };
        // The poll also covers a daemon too old to push shell changes.
        let shells: Vec<(Id, ShellState)> = {
            let panes = lock(&self.inner.panes);
            list.iter()
                .filter_map(|i| Some((panes.by_daemon.get(&i.pane)?.clone(), i.shell.clone())))
                .collect()
        };
        for (pane, shell) in shells {
            self.shell_changed(&pane, shell);
        }
        self.update_meta(|panes| {
            let mut changed = false;
            for info in list {
                let (Some(pane), Some(cwd)) =
                    (panes.by_daemon.get(&info.pane).cloned(), info.shell.cwd)
                else {
                    continue;
                };
                let rec = panes.meta.entry(pane).or_default();
                if rec.cwd.as_deref() != Some(cwd.as_str()) {
                    rec.cwd = Some(cwd);
                    changed = true;
                }
            }
            changed
        });
    }

    fn events(self, mut rx: Receiver<DaemonEvent>) {
        loop {
            while let Ok(event) = rx.recv() {
                match event {
                    DaemonEvent::Output { pane, data } => {
                        if let Some(p) = self.pane_of_daemon(pane)
                            && lock(&self.inner.views).output(&p, &data)
                        {
                            self.inner.queued.notify_one();
                        }
                    }
                    DaemonEvent::Attached { pane, .. } => {
                        if let Some(p) = self.pane_of_daemon(pane) {
                            lock(&self.inner.views).attached(&p);
                        }
                    }
                    DaemonEvent::Exited { pane, .. } => self.pane_exited(pane),
                    DaemonEvent::Shell { pane, shell } => {
                        if let Some(p) = self.pane_of_daemon(pane) {
                            self.shell_changed(&p, shell);
                        }
                    }
                    DaemonEvent::VarsChanged { side_tab } => {
                        if let Some(sink) = lock(&self.inner.context_sink).as_ref() {
                            sink(&side_tab);
                        }
                    }
                    DaemonEvent::Disconnected => break,
                }
            }
            *lock(&self.inner.daemon) = None;
            if self.inner.stopped.load(Ordering::SeqCst) {
                return;
            }
            rx = self.reconnect();
        }
    }

    /// A shell that exits closes its pane, and the last pane its tab.
    fn pane_exited(&self, daemon: PaneId) {
        match self.pane_of_daemon(daemon) {
            Some(pane) => {
                if let Err(e) = self.command(Command::ClosePane { pane_id: pane }) {
                    eprintln!("terminescu: closing the pane of daemon pane {daemon}: {e:?}");
                }
            }
            None => {
                {
                    let mut panes = lock(&self.inner.panes);
                    panes.early_exits.push_back(daemon);
                    if panes.early_exits.len() > EARLY_EXITS {
                        panes.early_exits.pop_front();
                    }
                }
                if let Some(c) = self.daemon() {
                    let _ = c.close(daemon);
                }
            }
        }
    }

    /// Reconnects until it works. Panes whose daemon pane is gone get a fresh shell in their
    /// last cwd.
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
                        lock(&self.inner.panes).clear_links();
                        let panes: Vec<Id> = lock(&self.inner.shells).keys().cloned().collect();
                        for pane in panes {
                            self.shell_restarted(&pane);
                        }
                    }
                    let orphans = std::mem::take(&mut lock(&self.inner.panes).orphans);
                    for d in orphans {
                        let _ = client.close(d);
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
                    for (pane, side) in ws.terminal_sides() {
                        let d = self.daemon_of(&pane);
                        if d.is_some_and(|d| live.contains(&d)) {
                            // A new connection has no subscriptions: attach views again.
                            let action = lock(&self.inner.views).reattach(&pane);
                            if let Some(action) = action {
                                self.act(&client, action);
                            }
                            continue;
                        }
                        let cwd = {
                            let mut panes = lock(&self.inner.panes);
                            panes.unlink(&pane);
                            // The Claude session died with its shell.
                            let rec = panes.meta.entry(pane.clone()).or_default();
                            rec.session = None;
                            rec.cwd.clone()
                        };
                        self.shell_restarted(&pane);
                        self.spawn_for(&client, &pane, &side, cwd.as_deref());
                    }
                    self.mark_status();
                    lock(&self.inner.instance).clone_from(&client.instance);
                    *lock(&self.inner.daemon) = Some(client);
                    self.save(&ws);
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

fn set_resume(ws: &mut Workspace, pane: &str, session: Option<String>) {
    for t in ws
        .windows
        .iter_mut()
        .flat_map(|w| w.side_tabs.iter_mut())
        .flat_map(|s| s.tabs.iter_mut())
    {
        if let Some(Layout::Pane { resume, .. }) = t.layout.leaf_mut(pane) {
            *resume = session;
            return;
        }
    }
}

/// The daemon work between two pane lists of (pane, side tab). New panes start in `cwd`.
fn diff_effects(before: &[(Id, Id)], after: &[(Id, Id)], cwd: Option<String>) -> Vec<Effect> {
    let old: HashMap<&Id, &Id> = before.iter().map(|(p, s)| (p, s)).collect();
    let new: HashMap<&Id, &Id> = after.iter().map(|(p, s)| (p, s)).collect();
    let mut out = Vec::new();
    for (pane, side) in after {
        match old.get(pane) {
            None => out.push(Effect::Spawn {
                pane: pane.clone(),
                side_tab: side.clone(),
                cwd: cwd.clone(),
            }),
            Some(was) if *was != side => out.push(Effect::SideTab {
                pane: pane.clone(),
                side_tab: side.clone(),
            }),
            Some(_) => {}
        }
    }
    for (pane, _) in before {
        if !new.contains_key(pane) {
            out.push(Effect::Close { pane: pane.clone() });
        }
    }
    out
}

#[cfg(test)]
mod session_id_tests {
    use super::is_session_id;

    #[test]
    fn a_session_id_never_reads_as_an_option() {
        assert!(is_session_id("00000000-0000-4000-8000-000000000001"));
        assert!(is_session_id("sess-0b5c"));
        assert!(!is_session_id("--dangerously-skip-permissions"));
        assert!(!is_session_id("-x"));
        assert!(!is_session_id("a b"));
        assert!(!is_session_id("a;rm"));
        assert!(!is_session_id(""));
        assert!(!is_session_id(&"a".repeat(129)));
        assert!(is_session_id(&"a".repeat(128)));
    }
}
