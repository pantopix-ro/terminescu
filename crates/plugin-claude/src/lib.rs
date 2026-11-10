//! The Claude Code plugin (add-plugin-architecture D1, D2). It owns everything that knows
//! Claude: hook payloads, the transcript format, `claude --resume` and Claude's projects
//! folder. The core drives agents through [`ClaudeAdapter`] and never names Claude.

pub mod agent;
pub mod hook;
pub mod transcript;

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
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

    fn uninstall(&self) -> Result<(), PluginError> {
        hook::uninstall(&self.settings)
            .map(drop)
            .map_err(|e| PluginError::Failed(e.to_string()))
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

/// The plugin: the adapter, the hook and the transcript reads of its pane view.
#[derive(Debug, Clone)]
pub struct ClaudePlugin {
    hook: ClaudeHook,
}

impl ClaudePlugin {
    /// The hook goes in Claude Code's user settings and runs `cli`.
    pub fn new(cli: PathBuf) -> Self {
        Self {
            hook: ClaudeHook {
                settings: hook::user_settings(),
                cli,
            },
        }
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

    fn on_event(&self, _event: &CoreEvent, _cx: &dyn PluginCx) {}

    /// `read {session, offset}`: the transcript entries since `offset` (task 4.4).
    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match method {
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
