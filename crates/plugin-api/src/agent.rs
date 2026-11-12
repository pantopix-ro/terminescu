//! A coding agent's state in its pane (task 4.2, D34), shared by the core and the adapters
//! (add-plugin-architecture D2). The core keeps the agents, attention and statuses; an
//! [`AgentAdapter`] turns one tool's hook events and transcript lines into changes.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    /// Started, no prompt yet.
    Idle,
    Running,
    /// Needs the person: a permission prompt or a question (amber).
    Waiting,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub session: String,
    pub transcript: Option<PathBuf>,
    pub state: AgentState,
    /// The agent's last reply, cut to a preview.
    pub last_message: Option<String>,
    /// While it waits: what for, as "Bash: npm publish" (add-claude-insights 2.3).
    pub reason: Option<String>,
    /// The name the person gave the session (Claude Code's `/rename`), from its log (task 8.88).
    pub title: Option<String>,
    /// The name the agent gave the session itself; [`Agent::title`] wins over it.
    pub ai_title: Option<String>,
}

/// What the UI gets per pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub session: String,
    pub state: AgentState,
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The session's name, which names the pane's tab (task 8.88).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One pane's agent changed; `agent: None` means the pane shows its shell again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentUpdate {
    pub pane: String,
    pub agent: Option<AgentView>,
}

impl Agent {
    pub fn view(&self) -> AgentView {
        AgentView {
            session: self.session.clone(),
            state: self.state,
            last_message: self.last_message.clone(),
            reason: self.reason.clone(),
            name: self.name().map(str::to_owned),
        }
    }

    /// The session's name: the person's, else the agent's own.
    pub fn name(&self) -> Option<&str> {
        self.title.as_deref().or(self.ai_title.as_deref())
    }
}

/// A session id the app may type into a shell: up to 128 of `[A-Za-z0-9_-]`, starting with a
/// letter or digit, so it never reads as an option.
pub fn is_session_id(s: &str) -> bool {
    s.len() <= 128
        && s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Facts a hook event carries besides the state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// The agent's reply at the end of a turn, for the extraction rules (D13).
    Reply(String),
    /// A file the agent wrote or edited with its own tools, absolute.
    Touched {
        path: String,
        created: bool,
        session: Option<String>,
        cwd: Option<String>,
    },
    /// The session a reboot resumes in this pane; `None` when the session ended.
    Session(Option<String>),
}

/// One coding tool (Strategy, D2). The runtime asks each adapter whether a hook event is its
/// own and drives the pane's agent through the one that accepts it.
pub trait AgentAdapter: Send + Sync {
    /// Stable id, as `claude`.
    fn id(&self) -> &'static str;
    /// The hook event comes from this tool.
    fn accepts(&self, event: &Value) -> bool;
    /// The agent after one hook event; `None` ends it.
    fn on_hook(&self, agent: Option<Agent>, event: &Value) -> Option<Agent>;
    /// One transcript line read while the agent runs or waits. True when the state changed.
    fn on_line(&self, agent: &mut Agent, line: &[u8]) -> bool;
    /// One transcript line read from the start, in any state, for the session's name (task
    /// 8.88). True when the name changed.
    fn on_name_line(&self, _agent: &mut Agent, _line: &[u8]) -> bool {
        false
    }
    /// Replies, touched files and the pane's session in a hook event.
    fn events(&self, event: &Value) -> Vec<AgentEvent>;
    /// The line that resumes `session` in a shell, typed without Enter by the caller.
    fn resume_command(&self, session: &str) -> Option<String>;
    /// The transcript file of a session the hooks never named.
    fn find_transcript(&self, session: &str) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
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
