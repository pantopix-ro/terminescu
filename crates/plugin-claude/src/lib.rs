//! The Claude Code plugin (add-plugin-architecture D1, D2). It owns everything that knows
//! Claude: hook payloads, the transcript format, `claude --resume` and Claude's projects
//! folder. The core drives agents through [`ClaudeAdapter`] and never names Claude.

pub mod agent;
pub mod baseline;
pub mod hook;
pub mod transcript;

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use terminescu_plugin_api::{
    Agent, AgentAdapter, AgentEvent, CoreEvent, Helper, Manifest, Plugin, PluginCx, PluginError,
    is_session_id,
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
}

/// What the plugin follows between events.
#[derive(Debug, Default)]
struct State {
    /// The session each pane runs, from its hook events.
    sessions: HashMap<String, String>,
    /// Sessions whose snapshot runs now, on a thread of its own.
    snapshotting: HashSet<String>,
}

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
        }
    }

    /// The same plugin with another `git`, for tests.
    pub fn with_git(mut self, git: PathBuf) -> Self {
        self.store.git = git;
        self
    }

    fn on_hook(&self, pane: &str, event: &Value) {
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
            CoreEvent::Hook { pane, event } => self.on_hook(pane, event),
            CoreEvent::AppLaunched => cx.schedule(PRUNE_FIRST, "prune"),
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
