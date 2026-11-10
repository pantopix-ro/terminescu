//! The Claude Code plugin (add-plugin-architecture D1, D2). It owns everything that knows
//! Claude: hook payloads, the transcript format, `claude --resume` and Claude's projects
//! folder. The core drives agents through [`ClaudeAdapter`] and never names Claude.

pub mod agent;
pub mod transcript;

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use terminescu_plugin_api::{
    Agent, AgentAdapter, AgentEvent, CoreEvent, Manifest, Plugin, PluginCx, PluginError,
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

/// The plugin: the adapter now; transcripts, helpers and its UI calls follow (tasks 3.3, 3.4).
#[derive(Debug, Default)]
pub struct ClaudePlugin;

impl Plugin for ClaudePlugin {
    fn manifest(&self) -> Manifest {
        Manifest::new(
            ID,
            "Claude",
            "Claude Code sessions: the amber row, transcripts, resume after reboot and the hook.",
        )
    }

    fn on_event(&self, _event: &CoreEvent, _cx: &dyn PluginCx) {}

    fn call(&self, method: &str, _args: Value, _cx: &dyn PluginCx) -> Result<Value, PluginError> {
        Err(PluginError::UnknownMethod(method.into()))
    }

    fn agent_adapter(&self) -> Option<Arc<dyn AgentAdapter>> {
        Some(Arc::new(ClaudeAdapter))
    }
}
