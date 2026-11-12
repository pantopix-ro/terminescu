//! The Claude Code plugin (add-plugin-architecture D1, D2). It owns everything that knows
//! Claude: hook payloads, the transcript format, `claude --resume` and Claude's projects
//! folder. The core drives agents through [`ClaudeAdapter`] and never names Claude.

pub mod agent;
pub mod baseline;
pub mod history;
pub mod hook;
pub mod insights;
pub mod memory;
pub mod settings;
pub mod transcript;

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use terminescu_plugin_api::baseline as baseline_store;
use terminescu_plugin_api::{
    Agent, AgentAdapter, AgentEvent, CoreEvent, Decoration, Helper, LineTail, Manifest, Plugin,
    PluginCx, PluginError, Target, is_session_id,
};

pub const ID: &str = "claude";

/// Claude Code as an agent (Strategy, D2).
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    /// `terminescu hook` forwards Claude Code's hook payloads, which all name their event.
    fn accepts(&self, event: &Value) -> bool {
        event.get("hook_event_name").is_some_and(Value::is_string)
    }

    fn on_hook(&self, agent: Option<Agent>, event: &Value) -> Option<Agent> {
        agent::on_hook(agent, event)
    }

    fn on_line(&self, agent: &mut Agent, line: &[u8]) -> bool {
        // The offset only places entries in a transcript pane; here it is unused.
        let mut changed = false;
        for e in transcript::parse_line(line, 0) {
            changed |= agent::on_entry(agent, &e);
        }
        changed
    }

    fn on_name_line(&self, agent: &mut Agent, line: &[u8]) -> bool {
        agent::on_name_line(agent, line)
    }

    fn events(&self, event: &Value) -> Vec<AgentEvent> {
        agent::events(event)
    }

    fn resume_command(&self, session: &str) -> Option<String> {
        is_session_id(session).then(|| format!("claude --resume {session}"))
    }

    fn find_transcript(&self, session: &str) -> Option<PathBuf> {
        if !is_session_id(session) {
            return None;
        }
        transcript::find_in(&transcript::projects_dir(), session)
    }
}

/// `terminescu hook` in Claude Code's user settings (D34, D36): the first-run entry and what
/// Uninstall Helpers takes out.
#[derive(Debug, Clone)]
pub struct ClaudeHook {
    /// Claude Code's `settings.json`.
    pub settings: PathBuf,
    /// The `terminescu` CLI the hook runs.
    pub cli: PathBuf,
    /// The baseline store, which Uninstall Helpers removes too.
    pub baselines: Option<PathBuf>,
}

impl Helper for ClaudeHook {
    fn id(&self) -> &'static str {
        "hook"
    }

    fn label(&self) -> &'static str {
        "Claude Code hook"
    }

    fn description(&self) -> &'static str {
        "Claude Code then tells Terminescu when a session starts, waits or ends. Without it the \
         terminals work, but Claude status and the transcript pane stay off."
    }

    fn installed(&self) -> bool {
        hook::present(&self.settings)
    }

    /// Idempotent: an entry for an older CLI path moves to this one.
    fn install(&self) -> Result<(), PluginError> {
        hook::install(&self.settings, &self.cli)
            .map(drop)
            .map_err(|e| PluginError::Failed(e.to_string()))
    }

    /// Takes the entries out, and the baselines with them (add-claude-insights 1.4).
    fn uninstall(&self) -> Result<(), PluginError> {
        hook::uninstall(&self.settings)
            .map(drop)
            .map_err(|e| PluginError::Failed(e.to_string()))?;
        if let Some(store) = &self.baselines {
            match std::fs::remove_dir_all(store) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(PluginError::Failed(e.to_string())),
            }
        }
        Ok(())
    }
}

#[derive(Debug, serde::Deserialize)]
struct ReadArgs {
    session: String,
    offset: u64,
}

/// What one transcript read returns to the pane.
#[derive(Debug, serde::Serialize)]
pub struct Chunk {
    pub entries: Vec<transcript::Entry>,
    /// Where the next read starts.
    pub offset: u64,
    /// The file shrank or was replaced: `entries` start over from the top.
    pub reset: bool,
}

/// The entries a session's transcript gained since `offset`.
pub fn read(cx: &dyn PluginCx, session: &str, offset: u64) -> Result<Chunk, PluginError> {
    if !is_session_id(session) {
        return Err(PluginError::BadArgs(format!(
            "{session} is not a session id"
        )));
    }
    let path = cx
        .agent_log(session)
        .ok_or_else(|| PluginError::Failed(format!("no transcript for session {session}")))?;
    let mut tail = transcript::Tail::at(path, offset);
    let entries = tail
        .read()
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    let reset = tail.offset() < offset || entries.first().is_some_and(|e| e.offset < offset);
    Ok(Chunk {
        entries,
        offset: tail.offset(),
        reset,
    })
}

/// The plugin: the adapter, the hook, the transcript reads of its pane view and the session
/// insights of add-claude-insights.
#[derive(Debug, Clone)]
pub struct ClaudePlugin {
    hook: ClaudeHook,
    store: baseline::Store,
    state: Arc<Mutex<State>>,
    /// Claude Code's own folder, for `CLAUDE.md` and the transcripts (3.3).
    config: PathBuf,
}

/// What the plugin follows between events.
#[derive(Debug, Default)]
struct State {
    /// The session each pane runs, from its hook events.
    sessions: HashMap<String, String>,
    /// Sessions whose snapshot runs now, on a thread of its own.
    snapshotting: HashSet<String>,
    /// What each pane's session shows (add-claude-insights group 2).
    live: HashMap<String, Live>,
    /// What each tab shows now: its lead pane's decorations (polish-claude-insights D1).
    shown: HashMap<String, Vec<Decoration>>,
    /// Hook events seen, so the panes of one tab can tell which spoke last.
    seq: u64,
    /// The tail timer that counts, and when it is due (D5). Older tokens are stale.
    timer: Option<(u64, Instant)>,
    timers: u64,
}

impl State {
    /// Each tab shows its lead pane's decorations: the pane with the latest hook event (D1).
    /// A tab changes only where its decorations differ.
    fn publish(&mut self, cx: &dyn PluginCx) {
        let tabs: HashMap<String, String> =
            cx.panes().into_iter().map(|p| (p.pane, p.tab)).collect();
        let mut lead: HashMap<&str, &Live> = HashMap::new();
        for (pane, live) in &self.live {
            if let Some(tab) = tabs.get(pane) {
                let best = lead.entry(tab).or_insert(live);
                if live.seq > best.seq {
                    *best = live;
                }
            }
        }
        let look = insights::Look::from_settings(
            cx.setting("meterThreshold"),
            cx.setting("contextWindows"),
        );
        let next: HashMap<String, Vec<Decoration>> = lead
            .into_iter()
            .map(|(tab, live)| (tab.to_string(), live.insight.decorations(tab, &look)))
            .filter(|(_, d)| !d.is_empty())
            .collect();
        for (tab, old) in &self.shown {
            if !next.contains_key(tab) {
                cx.clear_decorations(Some(&Target::Tab(tab.clone())));
            } else if next[tab] != *old {
                let new = &next[tab];
                let kept = old
                    .iter()
                    .all(|o| new.iter().any(|n| n.slot == o.slot && n.key == o.key));
                if !kept {
                    cx.clear_decorations(Some(&Target::Tab(tab.clone())));
                }
                for d in new.iter().filter(|d| !kept || !old.contains(d)) {
                    cx.set_decoration(d.clone());
                }
            }
        }
        for (tab, new) in &next {
            if !self.shown.contains_key(tab) {
                cx.clear_decorations(Some(&Target::Tab(tab.clone())));
                for d in new {
                    cx.set_decoration(d.clone());
                }
            }
        }
        self.shown = next;
    }

    /// Sets the tail timer: every second while a session works, else every 10 s. A timer due
    /// later than that is replaced; none runs while no session is followed.
    fn arm(&mut self, cx: &dyn PluginCx) {
        if self.live.is_empty() {
            self.timer = None;
            return;
        }
        let every = if self.live.values().any(|l| l.busy) {
            TAIL_BUSY
        } else {
            TAIL_IDLE
        };
        let at = Instant::now() + every;
        if self
            .timer
            .is_some_and(|(_, due)| due <= at + Duration::from_millis(50))
        {
            return;
        }
        self.timers += 1;
        self.timer = Some((self.timers, at));
        cx.schedule(every, &format!("{TAIL}{}", self.timers));
    }
}

/// One pane's session as its tab shows it.
#[derive(Debug)]
struct Live {
    session: String,
    insight: insights::Insight,
    tail: Option<LineTail>,
    /// The number of this pane's latest hook event.
    seq: u64,
    /// The session works now: from a prompt or a tool event until it stops.
    busy: bool,
    /// Reads in a row that found the transcript unchanged; a session that stays quiet for
    /// [`QUIET_READS`] is not working (an Esc interrupt sends no `Stop`).
    quiet: u32,
    /// The transcript's length and time at the last read.
    seen: Option<(u64, SystemTime)>,
}

impl Live {
    fn new(session: &str) -> Self {
        Self {
            session: session.into(),
            insight: insights::Insight::default(),
            tail: None,
            seq: 0,
            busy: false,
            quiet: 0,
            seen: None,
        }
    }

    /// Reads what the transcript gained, unless its length and time stayed. True when the
    /// todo list changed.
    fn catch_up(&mut self, cx: &dyn PluginCx) -> bool {
        if self.tail.is_none() {
            self.tail = cx.agent_log(&self.session).map(LineTail::new);
        }
        let Some(tail) = self.tail.as_mut() else {
            return false;
        };
        let now = std::fs::metadata(tail.path())
            .ok()
            .and_then(|m| Some((m.len(), m.modified().ok()?)));
        if now.is_some() && now == self.seen {
            self.quiet += 1;
            if self.quiet >= QUIET_READS {
                self.busy = false;
            }
            return false;
        }
        self.quiet = 0;
        self.seen = now;
        let before = self.insight.todos.clone();
        for (_, line) in tail.read().unwrap_or_default() {
            self.insight.on_line(&line);
        }
        self.insight.todos != before
    }
}

/// A transcript path a hook event names: absolute and a `.jsonl` file.
fn transcript_of(event: &Value) -> Option<PathBuf> {
    let path = PathBuf::from(event.get("transcript_path")?.as_str()?);
    (path.is_absolute() && path.extension().is_some_and(|e| e == "jsonl")).then_some(path)
}

/// How often a working session's transcript is read: the meter and todos follow within 2 s.
const TAIL_BUSY: Duration = Duration::from_secs(1);
/// Unchanged reads in a row after which a session counts as resting: 30 s at the busy rate.
const QUIET_READS: u32 = 30;
/// How often it is read while every session rests (polish-claude-insights D5).
const TAIL_IDLE: Duration = Duration::from_secs(10);
/// The tail timer's token, followed by its number.
const TAIL: &str = "tail:";

/// How often retention runs, and how soon after launch.
const PRUNE_EVERY: Duration = Duration::from_secs(6 * 3600);
const PRUNE_FIRST: Duration = Duration::from_secs(60);

fn lock(m: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn bad(e: impl std::fmt::Display) -> PluginError {
    PluginError::BadArgs(e.to_string())
}

impl ClaudePlugin {
    /// The hook goes in Claude Code's user settings and runs `cli`; baselines live under
    /// `support`, Terminescu's application support folder.
    pub fn new(cli: PathBuf, support: &Path) -> Self {
        let store = baseline::Store::new(support);
        Self {
            hook: ClaudeHook {
                settings: hook::user_settings(),
                cli,
                baselines: Some(store.dir.clone()),
            },
            store,
            state: Arc::default(),
            config: memory::config_dir(),
        }
    }

    /// Claude Code's folder somewhere else, as `CLAUDE_CONFIG_DIR` sets it; tests use a
    /// temporary one.
    pub fn with_config(mut self, dir: PathBuf) -> Self {
        self.hook.settings = dir.join("settings.json");
        self.config = dir;
        self
    }

    /// The same plugin with another `git`, for tests.
    pub fn with_git(mut self, git: PathBuf) -> Self {
        self.store.git = git;
        self
    }

    fn on_hook(&self, pane: &str, event: &Value, cx: &dyn PluginCx) {
        let name = event.get("hook_event_name").and_then(Value::as_str);
        let Some(session) = event
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|s| is_session_id(s))
        else {
            return;
        };
        let before = lock(&self.state)
            .sessions
            .insert(pane.to_string(), session.to_string());
        match name {
            Some("SessionStart") => self.session_started(session, before, event),
            Some("SessionEnd") => {
                lock(&self.state).sessions.remove(pane);
                if let Some(mut m) = self.store.meta(session) {
                    m.ended = Some(baseline::now());
                    let _ = self.store.set_meta(session, &m);
                }
            }
            _ => {}
        }
        if let Some(row) = cx
            .panes()
            .into_iter()
            .find(|p| p.pane == pane)
            .map(|p| p.side_tab)
        {
            history::on_hook(cx, &row, session, event, baseline::now());
        }
        self.follow(pane, session, name == Some("SessionEnd"), event, cx);
    }

    /// The pane's signals after one hook event: its tab's decorations and, while any session
    /// is followed, the tail timer.
    fn follow(&self, pane: &str, session: &str, ended: bool, event: &Value, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        if ended {
            if st.live.remove(pane).is_some() {
                st.publish(cx);
                cx.emit("insights", json!({ "pane": pane }));
            }
            st.arm(cx);
            return;
        }
        st.seq += 1;
        let seq = st.seq;
        let live = st
            .live
            .entry(pane.to_string())
            .or_insert_with(|| Live::new(session));
        if live.session != session {
            *live = Live::new(session);
        }
        live.seq = seq;
        if let Some(path) = transcript_of(event)
            && live.tail.as_ref().is_none_or(|t| t.path() != path)
        {
            live.tail = Some(LineTail::new(path));
            live.seen = None;
        }
        live.insight.on_hook(event);
        let name = event.get("hook_event_name").and_then(Value::as_str);
        match name {
            Some(
                "UserPromptSubmit" | "PostToolUse" | "PostToolUseFailure" | "PreCompact"
                | "SubagentStart",
            ) => {
                live.busy = true;
                live.quiet = 0;
            }
            Some("Stop") => live.busy = false,
            Some("Notification")
                if event.get("notification_type").and_then(Value::as_str)
                    == Some("idle_prompt") =>
            {
                live.busy = false
            }
            _ => {}
        }
        // A tool result or a finished turn may have changed files (3.2).
        if matches!(name, Some("PostToolUse" | "Stop")) {
            cx.emit("changed", json!({ "pane": pane }));
        }
        let todos = live.catch_up(cx);
        st.publish(cx);
        if todos {
            cx.emit("insights", json!({ "pane": pane }));
        }
        st.arm(cx);
    }

    /// The tail timer: every followed session catches up with its transcript. A stale timer,
    /// one a nearer timer replaced, does nothing.
    fn tick(&self, token: &str, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        if st.timer.map(|(n, _)| n.to_string()).as_deref() != Some(token) {
            return;
        }
        st.timer = None;
        let mut todos = Vec::new();
        for (pane, live) in st.live.iter_mut() {
            if live.catch_up(cx) {
                todos.push(pane.clone());
            }
        }
        // Also when no transcript grew: a pane may have moved to another tab.
        st.publish(cx);
        for pane in todos {
            cx.emit("insights", json!({ "pane": pane }));
        }
        st.arm(cx);
    }

    /// A new session gets its baseline (D1): a resumed, cleared or compacted one in the same
    /// pane uses the first one; any other takes a snapshot when it starts in a repository.
    fn session_started(&self, session: &str, before: Option<String>, event: &Value) {
        if self.store.meta(session).is_some() {
            return;
        }
        let Some(cwd) = event
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|c| Path::new(c).is_absolute())
        else {
            return;
        };
        let source = event.get("source").and_then(Value::as_str);
        let earlier = before
            .filter(|b| b != session && matches!(source, Some("resume" | "clear" | "compact")))
            .filter(|b| self.store.meta(b).is_some());
        let mut meta = baseline::Meta {
            cwd: cwd.into(),
            started: baseline::now(),
            ..Default::default()
        };
        if let Some(b) = earlier {
            meta.base = Some(self.store.meta(&b).and_then(|m| m.base).unwrap_or(b));
            let _ = self.store.set_meta(session, &meta);
            return;
        }
        if self.store.set_meta(session, &meta).is_err() {
            return;
        }
        if !lock(&self.state).snapshotting.insert(session.to_string()) {
            return;
        }
        // A large repository takes seconds: off the plugin's worker, so its budget holds.
        let key = session.to_string();
        let (store, state, session, cwd) = (
            self.store.clone(),
            self.state.clone(),
            session.to_string(),
            PathBuf::from(cwd),
        );
        let spawned = std::thread::Builder::new()
            .name("claude-snapshot".into())
            .spawn(move || {
                if let Some(repo) = store.repo_of(&cwd) {
                    let root = repo.root.display().to_string();
                    let _ = store.update_meta(&session, |m| m.root = Some(root));
                    if let Err(why) = store.snapshot(&session, &repo, baseline::SNAPSHOT_LIMIT) {
                        // Re-read: a SessionEnd may have landed during the snapshot.
                        let _ = store.update_meta(&session, |m| m.snapshot_failed = Some(why));
                    }
                }
                lock(&state).snapshotting.remove(&session);
            });
        if spawned.is_err() {
            lock(&self.state).snapshotting.remove(&key);
        }
    }

    /// Puts the per-file limit where the hook reads it (D3).
    fn write_file_cap(&self, cx: &dyn PluginCx) {
        if let Err(e) = baseline_store::set_file_cap(&self.store.dir, settings::file_cap(cx)) {
            eprintln!("terminescu: baseline file limit not saved: {e}");
        }
    }

    /// The session a pane runs now.
    fn session_of(&self, pane: &str) -> Option<String> {
        lock(&self.state).sessions.get(pane).cloned()
    }
}

#[derive(Debug, serde::Deserialize)]
struct SessionArgs {
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    pane: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct RowArgs {
    row: String,
    #[serde(default)]
    session: Option<String>,
    /// The row's shown shell, for its folder.
    #[serde(default)]
    pane: Option<String>,
    /// `memory`: "project" or "user".
    #[serde(default)]
    which: Option<String>,
}

impl RowArgs {
    /// The folder the row works in: its shell's cwd, else its newest session's.
    fn cwd(&self, cx: &dyn PluginCx) -> Option<PathBuf> {
        let panes = cx.panes();
        self.pane
            .as_deref()
            .and_then(|p| panes.iter().find(|i| i.pane == p && i.side_tab == self.row))
            .and_then(|i| i.cwd.clone())
            .or_else(|| {
                history::read(cx, &self.row)
                    .into_iter()
                    .next()
                    .map(|s| s.cwd)
            })
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    }
}

impl Plugin for ClaudePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            // A first read parses up to 8 MB of transcript. Overruns count toward a pause, and
            // a paused plugin takes its agents with it, so a slow disk gets room here.
            call_budget: std::time::Duration::from_secs(10),
            ..Manifest::new(
                ID,
                "Claude",
                "Claude Code sessions: the amber row, transcripts, resume after reboot and the hook.",
            )
        }
    }

    fn on_event(&self, event: &CoreEvent, cx: &dyn PluginCx) {
        match event {
            CoreEvent::Hook { pane, event } => self.on_hook(pane, event, cx),
            CoreEvent::AppLaunched => {
                cx.schedule(PRUNE_FIRST, "prune");
                self.write_file_cap(cx);
                // Switched on again: the decorations and timers went with the switch.
                let mut st = lock(&self.state);
                st.shown.clear();
                st.timer = None;
                st.publish(cx);
                st.arm(cx);
            }
            CoreEvent::Timer { token } if token.starts_with(TAIL) => {
                self.tick(&token[TAIL.len()..], cx)
            }
            CoreEvent::PaneClosed { pane } => {
                let mut st = lock(&self.state);
                st.sessions.remove(pane);
                if st.live.remove(pane).is_some() {
                    st.publish(cx);
                    st.arm(cx);
                }
            }
            CoreEvent::Timer { token } if token == "prune" => {
                let (keep, cap) = settings::limits(cx);
                self.store.prune(keep, cap, baseline::now());
                cx.schedule(PRUNE_EVERY, "prune");
            }
            // A lower limit in Settings › Claude applies now, outside the periodic chain.
            CoreEvent::Timer { token } if token == "prune-now" => {
                let (keep, cap) = settings::limits(cx);
                self.store.prune(keep, cap, baseline::now());
            }
            _ => {}
        }
    }

    /// `read {session, offset}`: the transcript entries since `offset` (task 4.4).
    /// `changes {session | pane}`: what the session changed (add-claude-insights 1.3).
    /// `baseText {session | pane, path}`: a file before the session.
    /// `todos {pane}`: the pane's session and its latest todo list, or null (2.2).
    /// `history {row}`: the row's sessions, newest first (3.1).
    /// `resume {row, session}`: a new tab in the session's folder with the resume line typed.
    /// `memory {row, pane?, which}`: opens the project's or the person's `CLAUDE.md` (3.3).
    /// `revealTranscripts {row, pane?}`: shows the row's newest transcript in Finder.
    /// `settings`, `setSetting {key, value}`: Settings › Claude (3.4).
    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match method {
            "changes" | "baseText" => {
                let args: SessionArgs = serde_json::from_value(args).map_err(bad)?;
                let session = args
                    .session
                    .or_else(|| args.pane.as_deref().and_then(|p| self.session_of(p)))
                    .filter(|s| is_session_id(s))
                    .ok_or_else(|| bad("no Claude session"))?;
                let out = if method == "changes" {
                    serde_json::to_value(self.store.changes(&session).map_err(PluginError::Failed)?)
                } else {
                    let path = args
                        .path
                        .filter(|p| Path::new(p).is_absolute())
                        .ok_or_else(|| bad("the path is absolute"))?;
                    serde_json::to_value(
                        self.store
                            .base_text(&session, Path::new(&path))
                            .map_err(PluginError::Failed)?,
                    )
                };
                let mut out = out.map_err(|e| PluginError::Failed(e.to_string()))?;
                if let Value::Object(m) = &mut out {
                    m.insert("session".into(), json!(session));
                }
                Ok(out)
            }
            "todos" => {
                let args: SessionArgs = serde_json::from_value(args).map_err(bad)?;
                let pane = args.pane.ok_or_else(|| bad("a pane"))?;
                let st = lock(&self.state);
                Ok(match st.live.get(&pane) {
                    Some(l) => json!({ "session": l.session, "todos": l.insight.todos }),
                    None => json!({ "session": null, "todos": null }),
                })
            }
            "history" => {
                let args: RowArgs = serde_json::from_value(args).map_err(bad)?;
                serde_json::to_value(history::read(cx, &args.row))
                    .map_err(|e| PluginError::Failed(e.to_string()))
            }
            "resume" => {
                let args: RowArgs = serde_json::from_value(args).map_err(bad)?;
                let id = args.session.ok_or_else(|| bad("a session"))?;
                let s = history::read(cx, &args.row)
                    .into_iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| bad(format!("{id} is not a session of this row")))?;
                let line = ClaudeAdapter
                    .resume_command(&s.id)
                    .ok_or_else(|| bad("not a session id"))?;
                cx.type_in_new_tab(&args.row, &s.cwd, &line)?;
                Ok(Value::Null)
            }
            "memory" => {
                let args: RowArgs = serde_json::from_value(args).map_err(bad)?;
                let path = match args.which.as_deref() {
                    Some("user") => memory::user_file(&self.config),
                    Some("project") => memory::project_file(
                        &args
                            .cwd(cx)
                            .ok_or_else(|| bad("the row's folder is not known"))?,
                        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
                    ),
                    _ => return Err(bad("which is project or user")),
                };
                let shown = path.display().to_string();
                cx.open_text(&args.row, &shown)?;
                Ok(json!({ "path": shown }))
            }
            "settings" => Ok(settings::view(cx)),
            "setSetting" => {
                let key = args
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("a key"))?;
                settings::set(cx, key, args.get("value").unwrap_or(&Value::Null))?;
                if matches!(key, settings::CAP_MB | settings::KEEP_DAYS) {
                    cx.schedule(Duration::ZERO, "prune-now");
                } else if key == settings::FILE_MB {
                    self.write_file_cap(cx);
                } else {
                    // The meter reads the new threshold or window at once.
                    lock(&self.state).publish(cx);
                }
                Ok(settings::view(cx))
            }
            "revealTranscripts" => {
                let args: RowArgs = serde_json::from_value(args).map_err(bad)?;
                let projects = self.config.join("projects");
                let newest = history::read(cx, &args.row)
                    .into_iter()
                    .find_map(|s| transcript::find_in(&projects, &s.id));
                let cwd = args.cwd(cx);
                let target = memory::reveal_target(&self.config, cwd.as_deref(), newest)
                    .ok_or_else(|| {
                        PluginError::Failed("Claude has written no transcript yet".into())
                    })?;
                let shown = target.display().to_string();
                cx.reveal(&shown)?;
                Ok(json!({ "path": shown }))
            }
            "read" => {
                let args: ReadArgs = serde_json::from_value(args)
                    .map_err(|e| PluginError::BadArgs(e.to_string()))?;
                let chunk = read(cx, &args.session, args.offset)?;
                serde_json::to_value(chunk).map_err(|e| PluginError::Failed(e.to_string()))
            }
            _ => Err(PluginError::UnknownMethod(method.into())),
        }
    }

    fn helpers(&self) -> Vec<Arc<dyn Helper>> {
        vec![Arc::new(self.hook.clone())]
    }

    fn agent_adapter(&self) -> Option<Arc<dyn AgentAdapter>> {
        Some(Arc::new(ClaudeAdapter))
    }
}
