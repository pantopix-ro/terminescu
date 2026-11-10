//! A Claude session's state in its pane, driven by hook events and, while it waits, by the
//! transcript (task 4.2, D34). Pure: the runtime feeds it events and entries and shows the result.

use crate::transcript::Entry;
use serde_json::Value;
use std::path::{Path, PathBuf};
use terminescu_plugin_api::{AgentEvent, is_session_id};

/// Longest `last_assistant_message` kept from `Stop`, in characters.
const LAST_MESSAGE_CHARS: usize = 2000;

// The agent types are shared with the adapters (add-plugin-architecture D2).
pub use terminescu_plugin_api::{Agent, AgentState, AgentUpdate, AgentView};

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
            reason: None,
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
        "PermissionRequest" => {
            a.state = AgentState::Waiting;
            a.reason = Some(reason_of(event));
        }
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
    if a.state != AgentState::Waiting {
        a.reason = None;
    }
    Some(a)
}

/// Longest waiting reason, in characters (add-claude-insights 2.3).
const REASON_CHARS: usize = 60;

/// What a permission request waits on, as "Bash: npm publish" or "Edit: src/retry.ts" (D7):
/// the tool and its main argument, a path inside the session's folder relative to it.
pub fn reason_of(event: &Value) -> String {
    let Some(tool) = str_of(event, "tool_name").filter(|t| !t.is_empty()) else {
        return "Waiting for you".into();
    };
    let input = event.get("tool_input");
    let arg = ["command", "file_path", "notebook_path", "url", "pattern"]
        .iter()
        .find_map(|k| input.and_then(|i| str_of(i, k)))
        .or_else(|| {
            input
                .and_then(Value::as_object)
                .and_then(|m| m.values().find_map(Value::as_str))
        });
    let arg = arg.map(|a| {
        let cwd = str_of(event, "cwd").filter(|c| Path::new(c).is_absolute());
        match cwd.and_then(|c| Path::new(a).strip_prefix(c).ok()) {
            Some(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
            _ => a.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    });
    let text = match arg.filter(|a| !a.is_empty()) {
        Some(a) => format!("{tool}: {a}"),
        None => tool.to_string(),
    };
    if text.chars().count() <= REASON_CHARS {
        text
    } else {
        let mut cut: String = text.chars().take(REASON_CHARS - 1).collect();
        cut.push('…');
        cut
    }
}

/// What a hook event says besides the state: Claude's reply at the end of a turn (D13), and the
/// file a `PostToolUse` for Write, Edit, MultiEdit or NotebookEdit touched (D30). A touched path
/// is absolute: a relative one joins the session's cwd, and without one it is dropped.
pub fn events(event: &Value) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    match str_of(event, "hook_event_name") {
        Some("Stop") => {
            if let Some(reply) = str_of(event, "last_assistant_message") {
                out.push(AgentEvent::Reply(reply.into()));
            }
        }
        Some("PostToolUse") => out.extend(touched(event)),
        _ => {}
    }
    let session = str_of(event, "session_id").filter(|s| is_session_id(s));
    match str_of(event, "hook_event_name") {
        Some("SessionEnd") => out.push(AgentEvent::Session(None)),
        _ => out.extend(session.map(|s| AgentEvent::Session(Some(s.into())))),
    }
    out
}

fn touched(event: &Value) -> Option<AgentEvent> {
    let tool = str_of(event, "tool_name")?;
    let input = event.get("tool_input")?;
    let path = match tool {
        "Write" | "Edit" | "MultiEdit" => str_of(input, "file_path")?,
        "NotebookEdit" => str_of(input, "notebook_path")?,
        _ => return None,
    };
    let cwd = str_of(event, "cwd").filter(|c| Path::new(c).is_absolute());
    let path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        Path::new(cwd?).join(path)
    };
    // Write reports `"type": "create"` for a new file and `"update"` over an old one.
    let created = tool == "Write"
        && event
            .get("tool_response")
            .and_then(|r| r.get("type"))
            .and_then(Value::as_str)
            == Some("create");
    Some(AgentEvent::Touched {
        path: path.display().to_string(),
        created,
        session: str_of(event, "session_id").map(String::from),
        cwd: cwd.map(String::from),
    })
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
        agent.reason = None;
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

    #[test]
    fn a_permission_request_names_what_it_waits_on() {
        let fixture = |name: &str| -> Value {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks/");
            serde_json::from_str(&std::fs::read_to_string(format!("{path}{name}")).unwrap())
                .unwrap()
        };
        assert_eq!(
            reason_of(&fixture("permissionrequest-bash.json")),
            "Bash: touch made-by-bash.txt"
        );
        assert_eq!(
            reason_of(&fixture("permissionrequest-edit.json")),
            "Edit: hello.txt"
        );
        let publish = ev(
            "PermissionRequest",
            json!({ "tool_name": "Bash", "tool_input": { "command": "npm  publish\n --access public" } }),
        );
        assert_eq!(reason_of(&publish), "Bash: npm publish --access public");
        let long = ev(
            "PermissionRequest",
            json!({ "tool_name": "WebFetch", "tool_input": { "url": format!("https://example.com/{}", "a".repeat(80)) } }),
        );
        let r = reason_of(&long);
        assert_eq!(r.chars().count(), 60);
        assert!(r.starts_with("WebFetch: https://example.com/") && r.ends_with('…'));
        assert_eq!(
            reason_of(&ev("PermissionRequest", json!({}))),
            "Waiting for you"
        );
        let a = on_hook(None, &publish).unwrap();
        assert_eq!(
            a.reason.as_deref(),
            Some("Bash: npm publish --access public")
        );
        let a = on_hook(Some(a), &ev("PostToolUse", json!({}))).unwrap();
        assert_eq!(a.reason, None, "an answered request drops its reason");
    }
}
