//! What a Claude hook event says besides the state: the reply at the end of a turn (D13) and
//! the file a file-editing tool touched (D30).

use serde_json::{Value, json};
use terminescu_plugin_api::AgentEvent;
use terminescu_plugin_claude::agent::events;

fn post(tool: &str, input: Value, response: Value) -> Value {
    json!({
        "hook_event_name": "PostToolUse",
        "session_id": "sess-0b5c",
        "cwd": "/Users/owner/acme-api",
        "tool_name": tool,
        "tool_input": input,
        "tool_response": response,
    })
}

fn touched(event: &Value) -> Option<(String, bool)> {
    events(event).into_iter().find_map(|e| match e {
        AgentEvent::Touched { path, created, .. } => Some((path, created)),
        _ => None,
    })
}

#[test]
fn an_event_names_the_session_and_session_end_clears_it() {
    let start = json!({"hook_event_name": "SessionStart", "session_id": "sess-0b5c"});
    assert_eq!(
        events(&start),
        [AgentEvent::Session(Some("sess-0b5c".into()))]
    );
    let prompt = json!({"hook_event_name": "UserPromptSubmit", "session_id": "sess-0b5c"});
    assert_eq!(
        events(&prompt),
        [AgentEvent::Session(Some("sess-0b5c".into()))]
    );
    // A tool call does not send the session again (polish-claude-insights D7).
    let tool = json!({"hook_event_name": "PreCompact", "session_id": "sess-0b5c"});
    assert!(events(&tool).is_empty());
    let end = json!({"hook_event_name": "SessionEnd", "session_id": "sess-0b5c"});
    assert_eq!(events(&end), [AgentEvent::Session(None)]);
    // A session id that could name a path is not one.
    let odd = json!({"hook_event_name": "UserPromptSubmit", "session_id": "../x"});
    assert!(events(&odd).is_empty());
}

#[test]
fn a_stop_carries_the_reply() {
    let stop =
        json!({"hook_event_name": "Stop", "last_assistant_message": "See https://example.com"});
    assert_eq!(
        events(&stop),
        [AgentEvent::Reply("See https://example.com".into())]
    );
    assert!(events(&json!({"hook_event_name": "Stop"})).is_empty());
}

#[test]
fn a_write_that_creates_is_added_and_a_relative_path_joins_the_cwd() {
    let write = post(
        "Write",
        json!({"file_path": "/Users/owner/acme-api/src/audit_export.rs"}),
        json!({"type": "create"}),
    );
    assert_eq!(
        touched(&write),
        Some(("/Users/owner/acme-api/src/audit_export.rs".into(), true))
    );
    let readme = post(
        "MultiEdit",
        json!({"file_path": "README.md", "edits": []}),
        json!({}),
    );
    assert_eq!(
        touched(&readme),
        Some(("/Users/owner/acme-api/README.md".into(), false))
    );
}

#[test]
fn only_file_editing_tools_after_they_ran_count() {
    let nb = post(
        "NotebookEdit",
        json!({"notebook_path": "/Users/owner/acme-api/a.ipynb"}),
        json!({}),
    );
    assert_eq!(touched(&nb).map(|t| t.1), Some(false));
    let over = post(
        "Write",
        json!({"file_path": "/tmp/x.md"}),
        json!({"type": "update"}),
    );
    assert_eq!(touched(&over).map(|t| t.1), Some(false));
    assert_eq!(
        touched(&post("Read", json!({"file_path": "/tmp/x.md"}), json!({}))),
        None
    );
    assert_eq!(
        touched(&post("Bash", json!({"command": "ls"}), json!({}))),
        None
    );
    let mut pre = post("Write", json!({"file_path": "/tmp/x.md"}), json!({}));
    pre["hook_event_name"] = json!("PreToolUse");
    assert_eq!(touched(&pre), None);
    // A relative path with no cwd has nothing to resolve against.
    let mut bare = post("Edit", json!({"file_path": "x.md"}), json!({}));
    bare.as_object_mut().unwrap().remove("cwd");
    assert_eq!(touched(&bare), None);
}
