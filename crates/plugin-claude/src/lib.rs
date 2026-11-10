//! The Claude Code plugin (add-plugin-architecture D1, D2). It owns everything that knows
//! Claude: hook payloads, the transcript format, `claude --resume` and Claude's projects
//! folder. The core drives agents through [`ClaudeAdapter`] and never names Claude.

pub mod agent;
pub mod baseline;
pub mod history;
pub mod hook;
pub mod insights;
pub mod memory;
pub mod transcript;

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    /// The tail timer is set.
    ticking: bool,
}

/// One pane's session as its tab shows it.
#[derive(Debug)]
struct Live {
    session: String,
    insight: insights::Insight,
    tail: Option<LineTail>,
    /// The tab the decorations went on, and what they were.
    tab: Option<String>,
    shown: Vec<Decoration>,
}

impl Live {
    fn new(session: &str) -> Self {
        Self {
            session: session.into(),
            insight: insights::Insight::default(),
            tail: None,
            tab: None,
            shown: Vec::new(),
        }
    }

    /// Reads what the transcript gained. True when the todo list changed.
    fn catch_up(&mut self, cx: &dyn PluginCx) -> bool {
        if self.tail.is_none() {
            self.tail = cx.agent_log(&self.session).map(LineTail::new);
        }
        let Some(tail) = self.tail.as_mut() else {
            return false;
        };
        let before = self.insight.todos.clone();
        for (_, line) in tail.read().unwrap_or_default() {
            self.insight.on_line(&line);
        }
        self.insight.todos != before
    }

    /// Puts the decorations on the pane's tab when they changed, or when the pane moved.
    fn publish(&mut self, pane: &str, cx: &dyn PluginCx) {
        let tab = cx
            .panes()
            .into_iter()
            .find(|p| p.pane == pane)
            .map(|p| p.tab);
        let look = insights::Look::from_settings(
            cx.setting("meterThreshold"),
            cx.setting("contextWindows"),
        );
        let next = tab
            .as_deref()
            .map(|t| self.insight.decorations(t, &look))
            .unwrap_or_default();
        if tab == self.tab && next == self.shown {
            return;
        }
        if let Some(old) = &self.tab {
            cx.clear_decorations(Some(&Target::Tab(old.clone())));
        }
        if let Some(t) = &tab
            && self.tab.as_ref() != Some(t)
        {
            cx.clear_decorations(Some(&Target::Tab(t.clone())));
        }
        for d in &next {
            cx.set_decoration(d.clone());
        }
        self.tab = tab;
        self.shown = next;
    }

    fn take_down(&mut self, cx: &dyn PluginCx) {
        if let Some(t) = self.tab.take() {
            cx.clear_decorations(Some(&Target::Tab(t)));
        }
        self.shown.clear();
    }
}

/// A transcript path a hook event names: absolute and a `.jsonl` file.
fn transcript_of(event: &Value) -> Option<PathBuf> {
    let path = PathBuf::from(event.get("transcript_path")?.as_str()?);
    (path.is_absolute() && path.extension().is_some_and(|e| e == "jsonl")).then_some(path)
}

/// How often a running session's transcript is read: the meter and todos follow within 2 s.
const TAIL_EVERY: Duration = Duration::from_secs(1);

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
    /// runs, the tail timer.
    fn follow(&self, pane: &str, session: &str, ended: bool, event: &Value, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        if ended {
            if let Some(mut live) = st.live.remove(pane) {
                live.take_down(cx);
                cx.emit("insights", json!({ "pane": pane }));
            }
            return;
        }
        let live = st
            .live
            .entry(pane.to_string())
            .or_insert_with(|| Live::new(session));
        if live.session != session {
            live.take_down(cx);
            *live = Live::new(session);
        }
        if let Some(path) = transcript_of(event)
            && live.tail.as_ref().is_none_or(|t| t.path() != path)
        {
            live.tail = Some(LineTail::new(path));
        }
        live.insight.on_hook(event);
        // A tool result or a finished turn may have changed files (3.2).
        if matches!(
            event.get("hook_event_name").and_then(Value::as_str),
            Some("PostToolUse" | "Stop")
        ) {
            cx.emit("changed", json!({ "pane": pane }));
        }
        let todos = live.catch_up(cx);
        live.publish(pane, cx);
        if todos {
            cx.emit("insights", json!({ "pane": pane }));
        }
        if !st.ticking {
            st.ticking = true;
            cx.schedule(TAIL_EVERY, "tail");
        }
    }

    /// The tail timer: every followed session catches up with its transcript.
    fn tick(&self, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        for (pane, live) in st.live.iter_mut() {
            let todos = live.catch_up(cx);
            live.publish(pane, cx);
            if todos {
                cx.emit("insights", json!({ "pane": pane }));
            }
        }
        st.ticking = !st.live.is_empty();
        if st.ticking {
            cx.schedule(TAIL_EVERY, "tail");
        }
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
                    let mut meta = store.meta(&session).unwrap_or_default();
                    meta.root = Some(repo.root.display().to_string());
                    let _ = store.set_meta(&session, &meta);
                    if let Err(why) = store.snapshot(&session, &repo, baseline::SNAPSHOT_LIMIT) {
                        meta.snapshot_failed = Some(why);
                        let _ = store.set_meta(&session, &meta);
                    }
                }
                lock(&state).snapshotting.remove(&session);
            });
        if spawned.is_err() {
            lock(&self.state).snapshotting.remove(&key);
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
                // Switched on again: the decorations went with the switch, so they go back up.
                let mut st = lock(&self.state);
                for (pane, live) in st.live.iter_mut() {
                    live.tab = None;
                    live.shown.clear();
                    live.publish(pane, cx);
                }
                if !st.live.is_empty() {
                    st.ticking = true;
                    cx.schedule(TAIL_EVERY, "tail");
                }
            }
            CoreEvent::Timer { token } if token == "tail" => self.tick(cx),
            CoreEvent::PaneClosed { pane } => {
                let mut st = lock(&self.state);
                st.sessions.remove(pane);
                if let Some(mut live) = st.live.remove(pane) {
                    live.take_down(cx);
                }
            }
            CoreEvent::Timer { token } if token == "prune" => {
                self.store
                    .prune(baseline::KEEP, baseline::CAP_BYTES, baseline::now());
                cx.schedule(PRUNE_EVERY, "prune");
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
                    ),
                    _ => return Err(bad("which is project or user")),
                };
                let shown = path.display().to_string();
                cx.open_text(&args.row, &shown)?;
                Ok(json!({ "path": shown }))
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
