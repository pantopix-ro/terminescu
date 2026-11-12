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

use crate::context::Variable;
use crate::daemon::{DaemonClient, DaemonError, DaemonEvent};
use crate::layout::Layout;
use crate::persist::{PaneRecord, Saved};
use crate::state::{Command, CommandError, Core, Id, Patch, PatchBatch, Snapshot, Workspace};
use crate::status::{self, Engine, Look, PaneFacts, Rule, Shown, Until};
use crate::views::{Action, Queued, Sink, ViewId, Views};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use terminescu_plugin_api::{
    Agent, AgentAdapter, AgentEvent, AgentState, AgentUpdate, CoreEvent, LineTail, Target,
};
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
/// Hears that an agent adapter panicked: the adapter's id and what it was doing.
pub type AdapterFailSink = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// An adapter failure caught in the runtime, reported once no lock is held.
type Failure = (&'static str, &'static str);

/// Runs one adapter call; a panic is caught, noted in `failed` and answered with `None`.
fn guarded<R>(
    adapter: &dyn AgentAdapter,
    what: &'static str,
    failed: &mut Vec<Failure>,
    f: impl FnOnce() -> R,
) -> Option<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => Some(r),
        Err(_) => {
            failed.push((adapter.id(), what));
            None
        }
    }
}

/// A pane's agent, the adapter that drives it, and its log tail while it works or waits.
struct Live {
    agent: Agent,
    adapter: Arc<dyn AgentAdapter>,
    tail: Option<LineTail>,
    /// The whole log, from its start and in any state, for the session's name (task 8.88).
    names: Option<LineTail>,
}
/// Receives each change of a pane's Claude session state (task 4.2).
pub type AgentSink = Box<dyn Fn(&AgentUpdate) + Send + Sync>;
/// Every tab's and side tab's status, sent whenever one changes (D15).
pub type StatusSink = Box<dyn Fn(&BTreeMap<Id, Shown>) + Send + Sync>;
/// Every terminal pane's label parts, sent whenever one changes (task 8.30).
pub type LabelSink = Box<dyn Fn(&BTreeMap<Id, crate::labels::PaneLabel>) + Send + Sync>;
/// Hears every command that ended after the runtime saw it start (task 8.40).
pub type CommandSink = Box<dyn Fn(CommandEnd) + Send + Sync>;

/// Records a start edge's time and turns an end edge into a [`CommandEnd`]. A run whose start
/// was not seen (it began before a restart, or both edges came in one read) has no length.
fn command_edges(
    starts: &mut HashMap<Id, std::time::Instant>,
    now: std::time::Instant,
    pane: &str,
    prev: Option<&ShellState>,
    next: &ShellState,
) -> Option<CommandEnd> {
    let prev = prev?;
    let started_at = (next.ended > prev.ended)
        .then(|| starts.remove(pane))
        .flatten();
    if next.started > prev.started && next.running {
        starts.insert(pane.into(), now);
    }
    let ms = now.saturating_duration_since(started_at?).as_millis() as u64;
    Some(CommandEnd {
        pane: pane.into(),
        command: next.command.clone(),
        exit: next.last_exit,
        ms,
    })
}

/// A command a pane's shell integration reported from start to end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEnd {
    pub pane: Id,
    pub command: Option<String>,
    pub exit: Option<i32>,
    pub ms: u64,
}
/// Hears which side tab's daemon variables changed (D21).
pub type ContextSink = Box<dyn Fn(&str) + Send + Sync>;
/// Where extraction matches go: side tab, the tab they came from, the matches (D13).
pub type FoundSink = Box<dyn Fn(&str, &str, Vec<crate::extract::Found>) + Send + Sync>;
/// Hears the documents whose text panes left the layout (D28).
pub type DocSink = Box<dyn Fn(&[Id]) + Send + Sync>;
/// Hears what plugins observe (add-plugin-architecture D3). It must not block.
pub type EventSink = Box<dyn Fn(CoreEvent) + Send + Sync>;
/// Runs the close a shell exit asks for, as the app runs the person's closes
/// (harden-plugin-host D1).
pub type ExitCloseSink = Arc<dyn Fn(Command) + Send + Sync>;
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

// A Claude session id as hooks report it; anything else never reaches a pane as typed text.
pub use terminescu_plugin_api::is_session_id;

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
    /// Last cwd and agent session per pane, kept across daemon restarts.
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
    /// The pane labels last sent (task 8.30).
    labels: BTreeMap<Id, crate::labels::PaneLabel>,
}

struct Inner {
    core: Mutex<Core>,
    effects: Mutex<()>,
    daemon: Mutex<Option<Arc<DaemonClient>>>,
    instance: Mutex<String>,
    panes: Mutex<Panes>,
    views: Mutex<Views>,
    /// The agent per pane, with the adapter that drives it and a log tail while it works or
    /// waits (D34, add-plugin-architecture D2).
    agents: Mutex<HashMap<Id, Live>>,
    /// The agent adapters of the enabled plugins, asked in order.
    adapters: Mutex<Vec<Arc<dyn AgentAdapter>>>,
    /// Told when an adapter panics, so its plugin's failure count can grow.
    adapter_fail: Mutex<Option<AdapterFailSink>>,
    agent_sink: Mutex<Option<AgentSink>>,
    /// Transcript files found by session id, so a polled transcript pane walks the projects
    /// folder once.
    paths: Mutex<HashMap<String, std::path::PathBuf>>,
    /// The last shell state per pane, from daemon pushes and the cwd poll.
    shells: Mutex<HashMap<Id, ShellState>>,
    /// Each pane's foreground process name, from the daemon (task 8.62).
    foreground: Mutex<HashMap<Id, String>>,
    status: Mutex<StatusState>,
    /// Set when something a status depends on changed; the status thread wakes on it.
    status_dirty: Mutex<bool>,
    status_wake: Condvar,
    status_sink: Mutex<Option<StatusSink>>,
    label_sink: Mutex<Option<LabelSink>>,
    command_sink: Mutex<Option<CommandSink>>,
    /// When each pane's running command started, as the runtime saw the start edge.
    command_starts: Mutex<HashMap<Id, std::time::Instant>>,
    /// Launcher side tabs and the tab of each of their commands (D24).
    launchers: Mutex<HashMap<Id, Vec<Option<Id>>>>,
    /// Panes that got a fresh shell at start, most likely after a reboot; launchers rerun there.
    respawned: Mutex<Vec<Id>>,
    context_sink: Mutex<Option<ContextSink>>,
    found_sink: Mutex<Option<FoundSink>>,
    /// Extraction over each daemon pane's output.
    scanners: Mutex<HashMap<PaneId, crate::extract::Scanner>>,
    doc_sink: Mutex<Option<DocSink>>,
    event_sink: Mutex<Option<EventSink>>,
    exit_close: Mutex<Option<ExitCloseSink>>,
    /// Plugin facts and variables (add-plugin-architecture D4).
    provided: Mutex<crate::provided::Provided>,
    /// Settings › Shell: new shells point `VISUAL` and `EDITOR` at `terminescu edit --wait`.
    editor: AtomicBool,
    /// Lines of history new shells keep in the daemon; 0 leaves the daemon's default (task 8.42).
    scrollback: AtomicU32,
    queued: Condvar,
    emit: Emit,
    connect: Connect,
    persist: Persist,
    stopped: AtomicBool,
}

/// The [`Runtime::command_in`] key for every new pane the command does not name.
pub const NEW_PANES: &str = "*";

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
        // A window left with no side tab restores as nothing to look at: it is dropped, and a
        // workspace with no side tab at all starts fresh, as a first start does.
        let saved = saved
            .map(|mut s| {
                s.workspace.windows.retain(|w| !w.side_tabs.is_empty());
                s
            })
            .filter(|s| !s.workspace.windows.is_empty());
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
                adapters: Mutex::default(),
                adapter_fail: Mutex::default(),
                agent_sink: Mutex::default(),
                paths: Mutex::default(),
                shells: Mutex::default(),
                foreground: Mutex::default(),
                status: Mutex::default(),
                status_dirty: Mutex::new(true),
                status_wake: Condvar::new(),
                status_sink: Mutex::default(),
                label_sink: Mutex::default(),
                command_sink: Mutex::default(),
                command_starts: Mutex::default(),
                launchers: Mutex::default(),
                respawned: Mutex::new(spawn.iter().map(|(p, _, _)| p.clone()).collect()),
                context_sink: Mutex::default(),
                found_sink: Mutex::default(),
                scanners: Mutex::default(),
                doc_sink: Mutex::default(),
                event_sink: Mutex::default(),
                exit_close: Mutex::default(),
                provided: Mutex::default(),
                editor: AtomicBool::new(editor),
                scrollback: AtomicU32::new(0),
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
        rt.thread("pane-poll", |me| me.poll_panes())?;
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

    /// The view's terminal cleared itself; the daemon's copy follows, so a reattach matches
    /// (task 8.44). An older daemon without the feature keeps its lines.
    pub fn view_clear(&self, view: ViewId, keep_screen: bool) {
        let pane = lock(&self.inner.views).pane_of(view);
        if let (Some(d), Some(client)) = (pane.and_then(|p| self.daemon_of(&p)), self.daemon()) {
            client.clear(d, keep_screen);
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
        self.command_in(cmd, &HashMap::new())
    }

    /// As [`Self::command`], with the cwd each new pane's shell starts in: a reopened tab's
    /// panes start where they were (task 8.87). [`NEW_PANES`] stands for any pane not named.
    pub fn command_in(
        &self,
        cmd: Command,
        cwds: &HashMap<Id, String>,
    ) -> Result<u64, CommandError> {
        let mut core = lock(&self.inner.core);
        let before = core.workspace().terminal_sides();
        let docs_before = core.workspace().doc_panes();
        // Typed text and the cwd a split inherits, read before the batch changes the layout.
        let (typed, inherit) = match &cmd {
            Command::ResumePane { pane_id } => {
                // The first adapter that can resume the session types its line (D2).
                let adapters = lock(&self.inner.adapters).clone();
                let mut failed = Vec::new();
                let session = core
                    .workspace()
                    .locate_pane(pane_id)
                    .and_then(|(_, t)| t.layout.resume_of(pane_id))
                    .filter(|s| is_session_id(s))
                    .and_then(|s| {
                        adapters.iter().find_map(|a| {
                            guarded(
                                a.as_ref(),
                                "panicked in resume_command",
                                &mut failed,
                                || a.resume_command(s),
                            )
                            .flatten()
                        })
                    })
                    .map(|line| (pane_id.clone(), format!("{line}\r")));
                self.report_failures(failed);
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
        for p in &batch.patches {
            match p {
                Patch::TabClosed { tab_id } => self.observe(CoreEvent::TabClosed {
                    tab: tab_id.clone(),
                }),
                Patch::SideTabClosed { side_tab_id } => self.observe(CoreEvent::SideTabClosed {
                    side_tab: side_tab_id.clone(),
                    data: Default::default(),
                }),
                _ => {}
            }
        }
        self.mark_status();
        let ws = core.workspace().clone();
        let guard = lock(&self.inner.effects);
        drop(core);
        let cwd = inherit.and_then(|p| self.live_cwd(&p));
        let mut effects = diff_effects(&before, &ws.terminal_sides(), cwd);
        for e in &mut effects {
            if let Effect::Spawn { pane, cwd, .. } = e
                && let Some(at) = cwds.get(pane).or_else(|| cwds.get(NEW_PANES))
            {
                *cwd = Some(at.clone());
            }
        }
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
        self.command_then_type_in(cmd, None, bytes)
    }

    /// As [`Self::command_then_type`], with the new pane's shell started in `cwd`. Nothing is
    /// typed when `bytes` is empty.
    pub fn command_then_type_in(
        &self,
        cmd: Command,
        cwd: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<(Option<Id>, bool), CommandError> {
        let before: std::collections::HashSet<Id> = self
            .workspace()
            .terminal_sides()
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        let cwds: HashMap<Id, String> = cwd
            .map(|c| HashMap::from([(NEW_PANES.to_string(), c.to_string())]))
            .unwrap_or_default();
        self.command_in(cmd, &cwds)?;
        let pane = self
            .workspace()
            .terminal_sides()
            .into_iter()
            .map(|(p, _)| p)
            .find(|p| !before.contains(p));
        let typed = !bytes.is_empty() && pane.as_ref().is_some_and(|p| self.type_into(p, bytes));
        Ok((pane, typed))
    }

    /// The cwd the shell integration reports now, else the last one seen.
    pub fn live_cwd(&self, pane: &str) -> Option<String> {
        self.live_cwds(&[pane.to_string()]).remove(pane)
    }

    /// [`Runtime::live_cwd`] of each pane, from one daemon list.
    pub fn live_cwds(&self, panes: &[Id]) -> HashMap<Id, String> {
        let live: HashMap<PaneId, String> = self
            .daemon()
            .and_then(|c| c.list().ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|i| Some((i.pane, i.shell.cwd?)))
            .collect();
        let meta = lock(&self.inner.panes);
        panes
            .iter()
            .filter_map(|p| {
                let now = meta.by_id.get(p).and_then(|d| live.get(d)).cloned();
                let cwd = now.or_else(|| meta.meta.get(p).and_then(|r| r.cwd.clone()))?;
                Some((p.clone(), cwd))
            })
            .collect()
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
                    self.observe(CoreEvent::PaneClosed { pane: pane.clone() });
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
                } => {
                    self.spawn_for(&client, &pane, &side_tab, cwd.as_deref());
                    self.observe(CoreEvent::PaneSpawned {
                        pane,
                        side_tab,
                        cwd,
                    });
                }
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
                    self.observe(CoreEvent::PaneClosed { pane });
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
            scrollback: Some(self.inner.scrollback.load(Ordering::SeqCst)).filter(|n| *n > 0),
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

    /// A hook event from a pane (D34). The session it names becomes the pane's session until
    /// `SessionEnd`, so a reboot can offer to resume it. A reply goes through the extraction
    /// rules (D13); the files the agent touched come back for the Files section (D30).
    pub fn hook(&self, daemon: PaneId, event: &serde_json::Value) -> Vec<crate::files::Touch> {
        let events = {
            // `panes` stays held while the agent changes, so a closing pane can't get an
            // agent back after `drop_agent` (lock order: panes, then agents).
            let panes = lock(&self.inner.panes);
            match panes.by_daemon.get(&daemon) {
                Some(pane) => {
                    let events = self.hook_agent(pane, event);
                    self.observe(CoreEvent::Hook {
                        pane: pane.clone(),
                        event: event.clone(),
                    });
                    events
                }
                None => Vec::new(),
            }
        };
        let mut touched = Vec::new();
        let mut session = None;
        for e in events {
            match e {
                AgentEvent::Reply(reply) => self.found(daemon, crate::extract::find(&reply)),
                AgentEvent::Session(s) => session = Some(s),
                touch @ AgentEvent::Touched { .. } => {
                    touched.extend(crate::files::Touch::of(touch))
                }
            }
        }
        let Some(next) = session else {
            return touched;
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
        touched
    }

    /// The transcript file of a Claude session: the one its hooks named, else the session's
    /// file in Claude's projects folder (task 4.4).
    pub fn transcript_path(&self, session: &str) -> Option<std::path::PathBuf> {
        if !is_session_id(session) {
            return None;
        }
        let known = lock(&self.inner.agents)
            .values()
            .find(|l| l.agent.session == session)
            .and_then(|l| l.agent.transcript.clone());
        if known.is_some() {
            return known;
        }
        if let Some(p) = lock(&self.inner.paths).get(session).filter(|p| p.is_file()) {
            return Some(p.clone());
        }
        let adapters = lock(&self.inner.adapters).clone();
        let mut failed = Vec::new();
        let found = adapters.iter().find_map(|a| {
            guarded(
                a.as_ref(),
                "panicked in find_transcript",
                &mut failed,
                || a.find_transcript(session),
            )
            .flatten()
        });
        self.report_failures(failed);
        let found = found?;
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
            .map(|(pane, l)| AgentUpdate {
                pane: pane.clone(),
                agent: Some(l.agent.view()),
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

    /// Where adapter panics go; the app counts them against the adapter's plugin.
    pub fn on_adapter_failure(&self, sink: AdapterFailSink) {
        *lock(&self.inner.adapter_fail) = Some(sink);
    }

    /// Reports caught adapter panics. A pause reached through the sink takes the runtime's own
    /// locks (it swaps the adapters), so it runs on its own thread and no caller waits on it.
    fn report_failures(&self, failed: Vec<Failure>) {
        if failed.is_empty() {
            return;
        }
        for (adapter, what) in &failed {
            eprintln!("terminescu: agent adapter {adapter} {what}");
        }
        let Some(sink) = lock(&self.inner.adapter_fail).clone() else {
            return;
        };
        let started = std::thread::Builder::new()
            .name("adapter-failure".into())
            .spawn(move || {
                for (adapter, what) in failed {
                    sink(adapter, what);
                }
            });
        if let Err(e) = started {
            eprintln!("terminescu: adapter failure not reported: {e}");
        }
    }

    /// The adapters of the enabled plugins, in plugin order. An agent whose adapter left goes,
    /// so a plugin switched off leaves no amber row behind.
    pub fn set_adapters(&self, adapters: Vec<Arc<dyn AgentAdapter>>) {
        // Lock order: agents, then adapters. A hook reads the adapters under the agents lock,
        // so no agent of a removed adapter comes back after the swap.
        let mut agents = lock(&self.inner.agents);
        let ids: HashSet<&str> = adapters.iter().map(|a| a.id()).collect();
        let gone: Vec<Id> = agents
            .iter()
            .filter(|(_, l)| !ids.contains(l.adapter.id()))
            .map(|(p, _)| p.clone())
            .collect();
        *lock(&self.inner.adapters) = adapters;
        let mut updates = Vec::new();
        for pane in gone {
            if agents.remove(&pane).is_some() {
                updates.push(AgentUpdate { pane, agent: None });
            }
        }
        let changed = !updates.is_empty();
        self.tell(updates);
        drop(agents);
        if changed {
            self.mark_status();
        }
    }

    /// Applies a hook event to the pane's agent and tells the UI when it shows differently.
    /// Returns the replies and touched files the event carries.
    fn hook_agent(&self, pane: &str, event: &serde_json::Value) -> Vec<AgentEvent> {
        // Lock order: agents, then adapters (see `set_adapters`).
        let mut agents = lock(&self.inner.agents);
        let adapters = lock(&self.inner.adapters).clone();
        let mut live = agents.remove(pane);
        let mut failed = Vec::new();
        let step = agent_step(&mut live, &adapters, event, &mut failed);
        let changed = step.as_ref().map(|(c, _)| *c);
        let after = live.as_ref().map(|l| l.agent.view());
        if let Some(l) = live {
            agents.insert(pane.into(), l);
        }
        let update = (changed == Some(true)).then(|| AgentUpdate {
            pane: pane.into(),
            agent: after,
        });
        let changed = update.is_some();
        self.tell(update.into_iter().collect());
        drop(agents);
        if changed {
            self.mark_status();
        }
        let events = step
            .and_then(|(_, a)| {
                guarded(a.as_ref(), "panicked in events", &mut failed, || {
                    a.events(event)
                })
            })
            .unwrap_or_default();
        self.report_failures(failed);
        events
    }

    /// Reads the logs of working and waiting sessions: a denied prompt, an interrupt or a
    /// finished turn ends them (D34). The pane's adapter reads each line.
    fn tail_agents(self) {
        while !self.inner.stopped.load(Ordering::SeqCst) {
            std::thread::sleep(TAIL_POLL);
            // Read the files with no lock held, then apply what was read to agents that still
            // wait on the same file.
            let waiting: Vec<(Id, String, LineTail)> = lock(&self.inner.agents)
                .iter()
                .filter_map(|(pane, l)| {
                    Some((pane.clone(), l.agent.session.clone(), l.tail.clone()?))
                })
                .collect();
            let read: Vec<_> = waiting
                .into_iter()
                .filter_map(|(pane, session, mut t)| {
                    let from = t.offset();
                    let lines = t.read().ok()?;
                    Some((pane, session, from, t, lines))
                })
                .collect();
            let mut agents = lock(&self.inner.agents);
            let mut updates = Vec::new();
            let mut failed = Vec::new();
            for (pane, session, from, t, lines) in read {
                let Some(l) = agents.get_mut(&pane) else {
                    continue;
                };
                if l.agent.session != session
                    || l.tail
                        .as_ref()
                        .is_none_or(|c| c.path() != t.path() || c.offset() != from)
                {
                    continue;
                }
                let adapter = l.adapter.clone();
                // A panicking adapter is counted and its line skipped; the tail goes on.
                let ended = lines.iter().any(|(_, line)| {
                    guarded(adapter.as_ref(), "panicked in on_line", &mut failed, || {
                        adapter.on_line(&mut l.agent, line)
                    })
                    .unwrap_or(false)
                });
                if ended {
                    l.tail = None;
                    updates.push(AgentUpdate {
                        pane,
                        agent: Some(l.agent.view()),
                    });
                } else {
                    l.tail = Some(t);
                }
            }
            let changed = !updates.is_empty();
            self.tell(updates);
            drop(agents);
            if changed {
                self.mark_status();
            }
            self.report_failures(failed);
            self.read_names();
        }
    }

    /// Reads what each session's log gained for its name (task 8.88), with no lock held while
    /// reading, and tells the UI of each name that changed.
    fn read_names(&self) {
        let open: Vec<(Id, String, LineTail)> = lock(&self.inner.agents)
            .iter()
            .filter_map(|(pane, l)| Some((pane.clone(), l.agent.session.clone(), l.names.clone()?)))
            .collect();
        let read: Vec<_> = open
            .into_iter()
            .filter_map(|(pane, session, mut t)| {
                let from = t.offset();
                let lines = t.read().ok()?;
                Some((pane, session, from, t, lines))
            })
            .collect();
        let mut agents = lock(&self.inner.agents);
        let mut updates = Vec::new();
        let mut failed = Vec::new();
        for (pane, session, from, t, lines) in read {
            let Some(l) = agents.get_mut(&pane) else {
                continue;
            };
            if l.agent.session != session
                || l.names
                    .as_ref()
                    .is_none_or(|c| c.path() != t.path() || c.offset() != from)
            {
                continue;
            }
            let adapter = l.adapter.clone();
            let mut renamed = false;
            for (_, line) in &lines {
                renamed |= guarded(
                    adapter.as_ref(),
                    "panicked in on_name_line",
                    &mut failed,
                    || adapter.on_name_line(&mut l.agent, line),
                )
                .unwrap_or(false);
            }
            l.names = Some(t);
            if renamed {
                updates.push(AgentUpdate {
                    pane,
                    agent: Some(l.agent.view()),
                });
            }
        }
        self.tell(updates);
        drop(agents);
        self.report_failures(failed);
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
        let ended = {
            // `shells` stays held while the engine sees the edge, so a push and a poll for the
            // same pane reach it in the order they were stored.
            let mut shells = lock(&self.inner.shells);
            let prev = shells.get(pane).cloned();
            if let Some(p) = &prev
                && (next.started < p.started || next.ended < p.ended || *p == next)
            {
                return;
            }
            if next.started > prev.as_ref().map_or(0, |p| p.started)
                && next.running
                && let Some(command) = &next.command
            {
                self.observe(CoreEvent::CommandStarted {
                    pane: pane.into(),
                    command: command.clone(),
                });
            }
            shells.insert(pane.into(), next.clone());
            lock(&self.inner.status)
                .engine
                .shell(pane, prev.as_ref(), &next);
            self.command_edges(pane, prev.as_ref(), &next)
        };
        if let Some(end) = &ended {
            self.observe(CoreEvent::CommandEnded {
                pane: end.pane.clone(),
                command: end.command.clone().unwrap_or_default(),
                exit: end.exit,
            });
        }
        if let Some(end) = ended
            && let Some(sink) = lock(&self.inner.command_sink).as_ref()
        {
            sink(end);
        }
        self.mark_status();
    }

    /// Times a command from the start edge to the end edge (see [`command_edges`]).
    fn command_edges(
        &self,
        pane: &str,
        prev: Option<&ShellState>,
        next: &ShellState,
    ) -> Option<CommandEnd> {
        let mut starts = lock(&self.inner.command_starts);
        command_edges(&mut starts, std::time::Instant::now(), pane, prev, next)
    }

    /// A pane got a new shell: its run counters start over, and statuses waiting for the old
    /// shell's next command go.
    fn shell_restarted(&self, pane: &str) {
        let mut shells = lock(&self.inner.shells);
        shells.remove(pane);
        lock(&self.inner.command_starts).remove(pane);
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

    /// Where the UI gets pane label changes; the current ones come from [`Runtime::labels`].
    /// Where a timed command's end goes (task 8.40).
    pub fn on_command_end(&self, sink: CommandSink) {
        *lock(&self.inner.command_sink) = Some(sink);
    }

    pub fn on_labels(&self, sink: LabelSink) {
        *lock(&self.inner.label_sink) = Some(sink);
    }

    /// Every terminal pane's label now (task 8.30).
    pub fn labels(&self) -> BTreeMap<Id, crate::labels::PaneLabel> {
        self.compute_statuses();
        lock(&self.inner.status).labels.clone()
    }

    /// Where the UI gets status changes; the current ones come from [`Runtime::statuses`].
    pub fn on_status(&self, sink: StatusSink) {
        *lock(&self.inner.status_sink) = Some(sink);
    }

    /// The opt-in editor setting; shells started from now on follow it (D27).
    pub fn set_editor(&self, on: bool) {
        self.inner.editor.store(on, Ordering::SeqCst);
    }

    /// Lines of history the daemon keeps for shells started from now on (task 8.42).
    pub fn set_scrollback(&self, lines: u32) {
        self.inner.scrollback.store(lines, Ordering::SeqCst);
    }

    /// Where the document store hears that the last pane of a document closed.
    pub fn on_docs_closed(&self, sink: DocSink) {
        *lock(&self.inner.doc_sink) = Some(sink);
    }

    pub fn on_event(&self, sink: EventSink) {
        *lock(&self.inner.event_sink) = Some(sink);
    }

    /// Where a shell exit's close goes; without one the runtime runs it itself.
    pub fn on_exit_close(&self, sink: ExitCloseSink) {
        *lock(&self.inner.exit_close) = Some(sink);
    }

    /// A plugin's fact on a target; statuses follow.
    pub fn set_fact(&self, plugin: &str, target: &Target, name: &str, value: Option<Value>) {
        if lock(&self.inner.provided).set_fact(plugin, target, name, value) {
            self.mark_status();
        }
    }

    /// A plugin's read-only variable on a target.
    pub fn set_provided_variable(
        &self,
        plugin: &str,
        target: &Target,
        name: &str,
        value: Option<String>,
    ) {
        lock(&self.inner.provided).set_variable(plugin, target, name, value);
    }

    /// Drops what a stopped plugin provided.
    pub fn forget_plugin(&self, plugin: &str) {
        if lock(&self.inner.provided).forget_plugin(plugin) {
            self.mark_status();
        }
    }

    /// A row's variables with the ones plugins provide after them; a name in `saved` wins.
    pub fn with_provided(&self, side_tab: &str, saved: Vec<Variable>) -> Vec<Variable> {
        let ws = self.workspace();
        lock(&self.inner.provided).with_variables(&ws, side_tab, saved)
    }

    fn observe(&self, event: CoreEvent) {
        let gone = match &event {
            CoreEvent::PaneClosed { pane } => Some(Target::Pane(pane.clone())),
            CoreEvent::TabClosed { tab } => Some(Target::Tab(tab.clone())),
            CoreEvent::SideTabClosed { side_tab, .. } => Some(Target::SideTab(side_tab.clone())),
            _ => None,
        };
        if let Some(t) = gone {
            lock(&self.inner.provided).forget(&t);
        }
        if let Some(sink) = lock(&self.inner.event_sink).as_ref() {
            sink(event);
        }
    }

    /// Where PR links, artifact links and branch names found in output and replies go.
    pub fn on_found(&self, sink: FoundSink) {
        *lock(&self.inner.found_sink) = Some(sink);
    }

    fn found(&self, daemon: PaneId, found: Vec<crate::extract::Found>) {
        if found.is_empty() {
            return;
        }
        if let Some((side, tab)) = self.place_of_daemon(daemon)
            && let Some(sink) = lock(&self.inner.found_sink).as_ref()
        {
            sink(&side, &tab, found);
        }
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
            .map(|(p, l)| (p.clone(), l.agent.state))
            .collect();
        let shells = lock(&self.inner.shells).clone();
        let foreground = lock(&self.inner.foreground).clone();
        let now = crate::now_ms();
        let terminals: Vec<Id> = ws.terminal_sides().into_iter().map(|(p, _)| p).collect();
        // Read before the status lock: no path takes `panes` under `status`.
        let home = std::env::var("HOME").ok();
        let saved: HashMap<Id, String> = lock(&self.inner.panes)
            .meta
            .iter()
            .filter_map(|(p, r)| Some((p.clone(), r.cwd.clone()?)))
            .collect();
        let holding = lock(&self.inner.provided).holding(&ws);
        let mut st = lock(&self.inner.status);
        st.engine.facts(&holding);
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
        let labels: BTreeMap<Id, crate::labels::PaneLabel> = terminals
            .iter()
            .map(|p| {
                let sh = shells.get(p);
                let cwd = sh
                    .and_then(|s| s.cwd.as_deref())
                    .or(saved.get(p).map(String::as_str));
                (
                    p.clone(),
                    crate::labels::PaneLabel {
                        process: foreground.get(p).cloned(),
                        ..crate::labels::label(
                            cwd,
                            sh.is_some_and(|s| s.running),
                            sh.and_then(|s| s.command.as_deref()),
                            sh.and_then(|s| s.title.as_deref()),
                            agents.contains_key(p),
                            home.as_deref(),
                        )
                    },
                )
            })
            .collect();
        if labels != st.labels {
            st.labels = labels.clone();
            if let Some(sink) = lock(&self.inner.label_sink).as_ref() {
                sink(&labels);
            }
        }
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

    /// Lists the panes once a second for the foreground process, so a tab's name follows what
    /// runs in it as Terminal.app's does (task 8.62). The same list gives the cwds every few
    /// seconds, so a reboot restarts shells where they were, or every second from a daemon
    /// too old to push shell changes, so statuses keep up.
    fn poll_panes(self) {
        let mut cwds_at = Instant::now();
        while !self.inner.stopped.load(Ordering::SeqCst) {
            std::thread::sleep(SHELL_POLL);
            let Some(client) = self.daemon() else {
                continue;
            };
            let Ok(list) = client.list() else {
                continue;
            };
            self.foreground_from(&list);
            if !client.pushes_shell || cwds_at.elapsed() >= CWD_POLL {
                cwds_at = Instant::now();
                self.cwds_from(list);
            }
        }
    }

    fn foreground_from(&self, list: &[PaneInfo]) {
        let next: HashMap<Id, String> = {
            let panes = lock(&self.inner.panes);
            list.iter()
                .filter_map(|i| {
                    let name = crate::labels::process_name(i.foreground.as_deref()?)?;
                    Some((panes.by_daemon.get(&i.pane)?.clone(), name))
                })
                .collect()
        };
        let changed = {
            let mut fg = lock(&self.inner.foreground);
            let changed = *fg != next;
            *fg = next;
            changed
        };
        if changed {
            self.mark_status();
        }
    }

    /// Reads every pane's cwd from the daemon now and saves the layout when one changed.
    pub fn refresh_cwds(&self) {
        if let Some(Ok(list)) = self.daemon().map(|c| c.list()) {
            self.cwds_from(list);
        }
    }

    fn cwds_from(&self, list: Vec<PaneInfo>) {
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
        let mut moved = Vec::new();
        self.update_meta(|panes| {
            for info in list {
                let (Some(pane), Some(cwd)) =
                    (panes.by_daemon.get(&info.pane).cloned(), info.shell.cwd)
                else {
                    continue;
                };
                let rec = panes.meta.entry(pane.clone()).or_default();
                if rec.cwd.as_deref() != Some(cwd.as_str()) {
                    rec.cwd = Some(cwd.clone());
                    moved.push(CoreEvent::CwdMoved { pane, cwd });
                }
            }
            !moved.is_empty()
        });
        for event in moved {
            self.observe(event);
        }
    }

    fn events(self, mut rx: Receiver<DaemonEvent>) {
        loop {
            while let Ok(event) = rx.recv() {
                match event {
                    DaemonEvent::Output { pane, data } => {
                        if let Some(p) = self.pane_of_daemon(pane) {
                            let queued = lock(&self.inner.views).output(&p, &data);
                            match queued {
                                Queued::No => {}
                                Queued::Ready => self.inner.queued.notify_one(),
                                Queued::Reattach(action) => {
                                    if let Some(client) = self.daemon() {
                                        self.act(&client, action);
                                    }
                                }
                            }
                        }
                        let found = lock(&self.inner.scanners)
                            .entry(pane)
                            .or_default()
                            .feed(&data);
                        self.found(pane, found);
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
        lock(&self.inner.scanners).remove(&daemon);
        match self.pane_of_daemon(daemon) {
            Some(pane) => {
                let close = Command::ClosePane { pane_id: pane };
                let sink = lock(&self.inner.exit_close).clone();
                match sink {
                    Some(sink) => sink(close),
                    None => {
                        if let Err(e) = self.command(close) {
                            eprintln!(
                                "terminescu: closing the pane of daemon pane {daemon}: {e:?}"
                            );
                        }
                    }
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

/// One hook event on a pane's agent (add-plugin-architecture D2): the pane's own adapter takes
/// it, else the first adapter that accepts it, and another tool's agent in the pane ends where
/// this one starts. Returns whether the agent shows differently and the adapter that took the
/// event, or `None` when no adapter took it and `live` stays as it was.
///
/// An adapter that panics (in `accepts` or `on_hook`) is noted in `failed` and the pane's agent
/// stays as it was.
fn agent_step(
    live: &mut Option<Live>,
    adapters: &[Arc<dyn AgentAdapter>],
    event: &Value,
    failed: &mut Vec<Failure>,
) -> Option<(bool, Arc<dyn AgentAdapter>)> {
    let own = live.as_ref().filter(|l| {
        guarded(l.adapter.as_ref(), "panicked in accepts", failed, || {
            l.adapter.accepts(event)
        })
        .unwrap_or(false)
    });
    let adapter = match own {
        Some(l) => l.adapter.clone(),
        None => adapters
            .iter()
            .find(|a| {
                guarded(a.as_ref(), "panicked in accepts", failed, || {
                    a.accepts(event)
                })
                .unwrap_or(false)
            })?
            .clone(),
    };
    let shown = live.as_ref().map(|l| l.agent.view());
    let (old, old_tail, old_names) = match live.as_ref() {
        Some(l) if l.adapter.id() == adapter.id() => {
            (Some(l.agent.clone()), l.tail.clone(), l.names.clone())
        }
        _ => (None, None, None),
    };
    let old_session = old.as_ref().map(|a| a.session.clone());
    let next = guarded(adapter.as_ref(), "panicked in on_hook", failed, || {
        adapter.on_hook(old, event)
    })?;
    *live = next.map(|agent| {
        // A working or waiting session needs its log read: a denial and an interrupt fire no
        // hook.
        let tail = match (agent.state, &agent.transcript) {
            (AgentState::Waiting | AgentState::Running, Some(path)) => match old_tail {
                Some(t) if t.path() == path => Some(t),
                _ => Some(LineTail::from_end(path)),
            },
            _ => None,
        };
        // A new session or log reads its name again from the top.
        let names = agent.transcript.as_ref().map(|path| match old_names {
            Some(n) if n.path() == path && old_session.as_deref() == Some(&agent.session) => n,
            _ => LineTail::new(path),
        });
        Live {
            agent,
            adapter: adapter.clone(),
            tail,
            names,
        }
    });
    Some((shown != live.as_ref().map(|l| l.agent.view()), adapter))
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
mod dock_effect_tests {
    use super::{Effect, diff_effects};
    use crate::state::{Command, Core, DockSide, Workspace};

    /// Task 8.32: a docked tab's panes and a pane made a tab keep their shells; across side
    /// tabs a pane only changes side tab.
    #[test]
    fn moving_panes_between_tabs_spawns_and_closes_nothing() {
        let mut n = 0;
        let mut c = Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        );
        c.first_start();
        let window = c.workspace().windows[0].id.clone();
        c.handle(Command::CreateSideTab {
            window_id: window,
            index: None,
        })
        .unwrap();
        let sides = c.workspace().windows[0].side_tabs.clone();
        let pane = sides[0].tabs[0].layout.panes()[0].clone();
        let before = c.workspace().terminal_sides();
        c.handle(Command::DockTab {
            tab_id: sides[1].tabs[0].id.clone(),
            pane_id: pane,
            side: DockSide::Right,
        })
        .unwrap();
        let effects = diff_effects(&before, &c.workspace().terminal_sides(), None);
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], Effect::SideTab { side_tab, .. } if *side_tab == sides[0].id)
        );

        let moved = sides[1].tabs[0].layout.panes()[0].clone();
        let before = c.workspace().terminal_sides();
        c.handle(Command::PaneToTab {
            pane_id: moved,
            index: 1,
        })
        .unwrap();
        assert!(diff_effects(&before, &c.workspace().terminal_sides(), None).is_empty());
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn state(started: u64, ended: u64, running: bool, exit: Option<i32>) -> ShellState {
        ShellState {
            integrated: true,
            command: Some("cargo build".into()),
            running,
            last_exit: exit,
            started,
            ended,
            ..Default::default()
        }
    }

    #[test]
    fn a_run_is_timed_from_its_start_edge_to_its_end_edge() {
        let mut starts = HashMap::new();
        let t0 = Instant::now();
        let idle = state(0, 0, false, None);
        let running = state(1, 0, true, None);
        assert_eq!(
            command_edges(&mut starts, t0, "p", Some(&idle), &running),
            None
        );
        let done = state(1, 1, false, Some(2));
        let end = command_edges(
            &mut starts,
            t0 + Duration::from_secs(42),
            "p",
            Some(&running),
            &done,
        )
        .unwrap();
        assert_eq!(
            (end.ms, end.exit, end.command.as_deref()),
            (42_000, Some(2), Some("cargo build"))
        );
        assert!(starts.is_empty());
    }

    #[test]
    fn a_run_whose_start_was_not_seen_has_no_length() {
        let mut starts = HashMap::new();
        let t0 = Instant::now();
        // The first read after a restart, and both edges in one read.
        assert_eq!(
            command_edges(&mut starts, t0, "p", None, &state(1, 0, true, None)),
            None
        );
        let both = state(1, 1, false, Some(0));
        assert_eq!(
            command_edges(&mut starts, t0, "p", Some(&state(0, 0, false, None)), &both),
            None
        );
    }

    #[test]
    fn an_end_and_the_next_start_in_one_read_time_both() {
        let mut starts = HashMap::new();
        let t0 = Instant::now();
        command_edges(
            &mut starts,
            t0,
            "p",
            Some(&state(0, 0, false, None)),
            &state(1, 0, true, None),
        );
        let next = state(2, 1, true, Some(0));
        let end = command_edges(
            &mut starts,
            t0 + Duration::from_secs(5),
            "p",
            Some(&state(1, 0, true, None)),
            &next,
        )
        .unwrap();
        assert_eq!(end.ms, 5_000);
        assert!(starts.contains_key("p"));
    }
}

#[cfg(test)]
mod adapter_tests {
    use super::{Live, agent_step, guarded};
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use std::sync::Arc;
    use terminescu_plugin_api::{Agent, AgentAdapter, AgentEvent, AgentState};

    /// A tool whose hook events carry `tool` and `state`; a log line `done` ends its turn.
    struct Fake(&'static str);

    impl AgentAdapter for Fake {
        fn id(&self) -> &'static str {
            self.0
        }
        fn accepts(&self, event: &Value) -> bool {
            event["tool"] == self.0
        }
        fn on_hook(&self, agent: Option<Agent>, event: &Value) -> Option<Agent> {
            if event["boom"] == true {
                panic!("a fake adapter fails");
            }
            let state = match event["state"].as_str()? {
                "running" => AgentState::Running,
                "waiting" => AgentState::Waiting,
                "done" => AgentState::Done,
                _ => return None,
            };
            let mut a = agent.unwrap_or(Agent {
                session: "s-1".into(),
                transcript: Some(PathBuf::from("/tmp/terminescu-fake.log")),
                state,
                last_message: None,
                reason: None,
                title: None,
                ai_title: None,
            });
            a.state = state;
            Some(a)
        }
        fn on_line(&self, agent: &mut Agent, line: &[u8]) -> bool {
            if line == b"boom" {
                panic!("a fake adapter fails");
            }
            let ends = line == b"done" && agent.state != AgentState::Done;
            if ends {
                agent.state = AgentState::Done;
            }
            ends
        }
        fn events(&self, _: &Value) -> Vec<AgentEvent> {
            Vec::new()
        }
        fn resume_command(&self, session: &str) -> Option<String> {
            Some(format!("fake resume {session}"))
        }
        fn find_transcript(&self, _: &str) -> Option<PathBuf> {
            None
        }
    }

    fn adapters() -> Vec<Arc<dyn AgentAdapter>> {
        vec![Arc::new(Fake("one")), Arc::new(Fake("two"))]
    }

    fn step(mut live: Option<Live>, event: Value) -> (Option<Live>, bool) {
        let (changed, _) = agent_step(&mut live, &adapters(), &event, &mut Vec::new())
            .expect("no adapter took it");
        (live, changed)
    }

    #[test]
    fn the_runtime_drives_an_agent_through_its_adapter() {
        let (live, changed) = step(None, json!({ "tool": "one", "state": "running" }));
        let mut live = live.unwrap();
        assert!(changed);
        assert_eq!(
            (live.adapter.id(), live.agent.state),
            ("one", AgentState::Running)
        );
        // Working: its log is read; the adapter's line ends the turn.
        assert!(live.tail.is_some());
        assert!(live.adapter.clone().on_line(&mut live.agent, b"done"));
        let (live, changed) = step(Some(live), json!({ "tool": "one", "state": "done" }));
        assert!(!changed);
        assert!(live.unwrap().tail.is_none());
    }

    #[test]
    fn the_name_reader_starts_at_the_top_and_stays_in_any_state() {
        let (live, _) = step(None, json!({ "tool": "one", "state": "done" }));
        let mut live = live.unwrap();
        // Done: the turn's log is not read, the name's is, from its first byte (task 8.88).
        assert!(live.tail.is_none());
        let names = live.names.as_mut().expect("a name reader");
        assert_eq!(names.offset(), 0);
        *names = super::LineTail::at(names.path(), 42);
        let (live, _) = step(Some(live), json!({ "tool": "one", "state": "running" }));
        assert_eq!(
            live.as_ref().unwrap().names.as_ref().unwrap().offset(),
            42,
            "kept across events"
        );
        // Another tool in the pane reads its own log from the top again.
        let (live, _) = step(live, json!({ "tool": "two", "state": "running" }));
        assert_eq!(live.unwrap().names.unwrap().offset(), 0);
    }

    #[test]
    fn an_event_no_adapter_takes_leaves_the_pane_alone() {
        let (mut live, _) = step(None, json!({ "tool": "one", "state": "waiting" }));
        let other = json!({ "hook_event_name": "Stop" });
        assert!(agent_step(&mut live, &adapters(), &other, &mut Vec::new()).is_none());
        assert_eq!(live.unwrap().agent.state, AgentState::Waiting);
        // With no adapter at all (the plugin is off), nothing starts.
        let mut none = None;
        let waiting = json!({ "tool": "one", "state": "waiting" });
        assert!(agent_step(&mut none, &[], &waiting, &mut Vec::new()).is_none());
        assert!(none.is_none());
    }

    #[test]
    fn another_tool_in_the_pane_replaces_the_agent() {
        let (live, _) = step(None, json!({ "tool": "one", "state": "waiting" }));
        let (live, changed) = step(live, json!({ "tool": "two", "state": "running" }));
        assert!(changed);
        let live = live.unwrap();
        assert_eq!(
            (live.adapter.id(), live.agent.state),
            ("two", AgentState::Running)
        );
    }

    #[test]
    fn a_panicking_adapter_is_caught_and_the_agent_stays() {
        let (live, _) = step(None, json!({ "tool": "one", "state": "waiting" }));
        let mut live = live;
        let mut failed = Vec::new();
        let boom = json!({ "tool": "one", "state": "running", "boom": true });
        assert!(agent_step(&mut live, &adapters(), &boom, &mut failed).is_none());
        assert_eq!(failed, [("one", "panicked in on_hook")]);
        let kept = live.as_ref().expect("the agent was kept");
        assert_eq!(kept.agent.state, AgentState::Waiting);
        assert!(kept.tail.is_some(), "its tail was kept too");
        // The next event is handled as usual.
        let next = json!({ "tool": "one", "state": "running" });
        let (changed, _) = agent_step(&mut live, &adapters(), &next, &mut failed).unwrap();
        assert!(changed);
        assert_eq!(live.unwrap().agent.state, AgentState::Running);
    }

    #[test]
    fn a_panicking_line_reader_does_not_stop_the_next_line() {
        let (live, _) = step(None, json!({ "tool": "one", "state": "running" }));
        let mut live = live.unwrap();
        let adapter = live.adapter.clone();
        let mut failed = Vec::new();
        let first = guarded(adapter.as_ref(), "panicked in on_line", &mut failed, || {
            adapter.on_line(&mut live.agent, b"boom")
        });
        assert!(first.is_none());
        assert_eq!(failed, [("one", "panicked in on_line")]);
        let second = guarded(adapter.as_ref(), "panicked in on_line", &mut failed, || {
            adapter.on_line(&mut live.agent, b"done")
        });
        assert_eq!(second, Some(true));
    }
}
