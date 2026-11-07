//! A Claude session's state in its pane, driven by hook events and, while it waits, by the
//! transcript (task 4.2, D34). Pure: the runtime feeds it events and entries and shows the result.

use crate::runtime::is_session_id;
use crate::transcript::Entry;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

/// Longest `last_assistant_message` kept from `Stop`, in characters.
const LAST_MESSAGE_CHARS: usize = 2000;

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
    /// Claude's last reply from `Stop`, cut to a preview.
    pub last_message: Option<String>,
}

/// What the UI gets per pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub session: String,
    pub state: AgentState,
    pub last_message: Option<String>,
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
        }
    }
}

fn str_of<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get(key).and_then(Value::as_str)
}

/// A transcript path Claude sent: absolute and a `.jsonl` file, else ignored.
fn transcript_of(event: &Value) -> Option<PathBuf> {
    let path = PathBuf::from(str_of(event, "transcript_path")?);
    (path.is_absolute() && path.extension().is_some_and(|e| e == "jsonl")).then_some(path)
}

/// The agent after one hook event. Any event naming a session adopts it, so a session that
/// started before the app still shows up; an event with a new session replaces the old one
/// (`/clear` starts a new session in the same pane).
pub fn on_hook(agent: Option<Agent>, event: &Value) -> Option<Agent> {
    let name = str_of(event, "hook_event_name")?;
    let session = str_of(event, "session_id").filter(|s| is_session_id(s));
    if name == "SessionEnd" {
        // Only the session that ends drops the icon.
        return match (&agent, session) {
            (Some(a), Some(s)) if a.session != s => agent,
            _ => None,
        };
    }
    let Some(session) = session else {
        return agent;
    };
    let mut a = match agent {
        Some(a) if a.session == session => a,
        _ => Agent {
            session: session.into(),
            transcript: None,
            state: AgentState::Idle,
            last_message: None,
        },
    };
    if let Some(t) = transcript_of(event) {
        a.transcript = Some(t);
    }
    match name {
        "SessionStart" => {}
        "UserPromptSubmit" => {
            a.state = AgentState::Running;
            a.last_message = None;
        }
        "PermissionRequest" => a.state = AgentState::Waiting,
        "Notification" => {
            // `idle_prompt` comes 60 s after Stop: a finished session is not a request.
            // `permission_prompt` follows `PermissionRequest` by seconds and often lands after
            // a fast approval, so only a question (`elicitation_dialog`) sets Waiting. Each hook
            // posts on its own connection, so arrival order is not guaranteed.
            if str_of(event, "notification_type") == Some("elicitation_dialog") {
                a.state = AgentState::Waiting;
            }
        }
        // A tool result that lands after `Stop` belongs to the finished turn.
        "PostToolUse" | "PostToolUseFailure" | "PermissionDenied"
            if a.state != AgentState::Done =>
        {
            a.state = AgentState::Running
        }
        "Stop" => {
            a.state = AgentState::Done;
            a.last_message = str_of(event, "last_assistant_message")
                .map(|m| m.chars().take(LAST_MESSAGE_CHARS).collect());
        }
        _ => {}
    }
    Some(a)
}

/// A transcript line read while the agent works or waits. A denied prompt and an interrupt fire
/// no hook, so the transcript ends Waiting and Running. Returns whether the state changed.
pub fn on_entry(agent: &mut Agent, entry: &Entry) -> bool {
    let ends = match agent.state {
        AgentState::Waiting => entry.kind.ends_waiting(),
        AgentState::Running => entry.kind.ends_running(),
        _ => false,
    };
    if ends {
        agent.state = AgentState::Done;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(name: &str, extra: Value) -> Value {
        let mut v = json!({ "hook_event_name": name, "session_id": "sess-0b5c" });
        if let (Value::Object(m), Value::Object(e)) = (&mut v, extra) {
            m.extend(e);
        }
        v
    }

    fn run(events: &[Value]) -> Option<AgentState> {
        events.iter().fold(None, on_hook).map(|a| a.state)
    }

    #[test]
    fn a_late_permission_notification_does_not_bring_back_waiting() {
        let late = ev(
            "Notification",
            json!({ "notification_type": "permission_prompt" }),
        );
        let events = [
            ev("UserPromptSubmit", json!({})),
            ev("PermissionRequest", json!({})),
            ev("PostToolUse", json!({})),
            late,
        ];
        assert_eq!(run(&events), Some(AgentState::Running));
    }

    #[test]
    fn a_question_waits_and_a_late_tool_result_keeps_done() {
        let ask = ev(
            "Notification",
            json!({ "notification_type": "elicitation_dialog" }),
        );
        assert_eq!(
            run(&[ev("UserPromptSubmit", json!({})), ask]),
            Some(AgentState::Waiting)
        );
        let events = [
            ev("UserPromptSubmit", json!({})),
            ev("Stop", json!({})),
            ev("PostToolUse", json!({})),
        ];
        assert_eq!(run(&events), Some(AgentState::Done));
    }

    #[test]
    fn an_interrupt_in_the_transcript_ends_running() {
        let line = br#"{"type":"user","uuid":"u-1","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#;
        let entries = crate::transcript::parse_line(line, 0);
        let mut a = on_hook(None, &ev("UserPromptSubmit", json!({}))).unwrap();
        assert_eq!(a.state, AgentState::Running);
        assert!(entries.iter().any(|e| on_entry(&mut a, e)));
        assert_eq!(a.state, AgentState::Done);
        // At its prompt the same line changes nothing.
        let mut idle = on_hook(None, &ev("SessionStart", json!({}))).unwrap();
        assert!(!entries.iter().any(|e| on_entry(&mut idle, e)));
        assert_eq!(idle.state, AgentState::Idle);
    }
}
