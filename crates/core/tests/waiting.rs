//! Replays the hook order recorded in spike 0.5 through the agent state (task 4.2, D34).

use serde_json::{Value, json};
use std::path::Path;
use terminescu_core::agent::{Agent, AgentState, on_entry, on_hook};
use terminescu_core::transcript::parse_line;

const SESSION: &str = "00000000-0000-4000-8000-000000000001";

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks"))
}

fn state(agent: &Option<Agent>) -> &'static str {
    match agent.as_ref().map(|a| a.state) {
        None => "shell",
        Some(AgentState::Idle) => "idle",
        Some(AgentState::Running) => "running",
        Some(AgentState::Waiting) => "waiting",
        Some(AgentState::Done) => "done",
    }
}

/// The hook JSON for one step: the recorded fixture when there is one, else the fields the
/// step names.
fn event_for(step: &Value) -> Option<Value> {
    let name = step.get("event")?.as_str()?;
    if name == "PreToolUse" {
        // Not registered (D10): PreToolUse fires before the prompt and cannot clear it.
        return None;
    }
    let mut event = json!({ "hook_event_name": name, "session_id": SESSION });
    for key in ["notification_type", "reason", "tool"] {
        if let Some(v) = step.get(key) {
            let field = if key == "tool" { "tool_name" } else { key };
            event[field] = v.clone();
        }
    }
    Some(event)
}

#[test]
fn every_recorded_sequence_ends_in_the_expected_states() {
    let raw = std::fs::read_to_string(fixtures().join("sequences.json")).unwrap();
    let all: Value = serde_json::from_str(&raw).unwrap();
    let mut checked = 0;
    for (case, steps) in all["cases"].as_object().unwrap() {
        let mut agent: Option<Agent> = None;
        for (i, step) in steps.as_array().unwrap().iter().enumerate() {
            if let Some(file) = step.get("transcript").and_then(Value::as_str) {
                let text = std::fs::read(fixtures().join(file)).unwrap();
                let mut offset = 0;
                for line in text.split_inclusive(|b| *b == b'\n') {
                    if let Some(a) = agent.as_mut() {
                        for entry in parse_line(line, offset) {
                            on_entry(a, &entry);
                        }
                    }
                    offset += line.len() as u64;
                }
            } else if let Some(event) = event_for(step) {
                agent = on_hook(agent, &event);
            }
            assert_eq!(state(&agent), step["expect"], "{case} step {i}: {step}");
            checked += 1;
        }
    }
    assert!(checked >= 20, "only {checked} steps ran");
}

#[test]
fn the_recorded_payloads_drive_the_same_states() {
    let load = |f: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(fixtures().join(f)).unwrap()).unwrap()
    };
    let agent = on_hook(None, &load("sessionstart.json"));
    assert_eq!(state(&agent), "idle");
    let a = agent.as_ref().unwrap();
    assert!(
        a.transcript
            .as_ref()
            .unwrap()
            .ends_with(format!("{SESSION}.jsonl"))
    );
    let agent = on_hook(agent, &load("userpromptsubmit.json"));
    let agent = on_hook(agent, &load("permissionrequest-bash.json"));
    assert_eq!(state(&agent), "waiting");
    let agent = on_hook(agent, &load("notification-permission_prompt.json"));
    let agent = on_hook(agent, &load("posttooluse-bash.json"));
    assert_eq!(state(&agent), "running");
    let agent = on_hook(agent, &load("stop.json"));
    assert_eq!(state(&agent), "done");
    let agent = on_hook(agent, &load("notification-idle_prompt.json"));
    assert_eq!(state(&agent), "done");
    assert!(on_hook(agent, &load("sessionend.json")).is_none());
}

#[test]
fn a_new_session_replaces_the_old_and_bad_ids_are_ignored() {
    let start = |s: &str| json!({ "hook_event_name": "SessionStart", "session_id": s });
    let agent = on_hook(None, &start("sess-0b5c"));
    let agent = on_hook(
        agent,
        &json!({ "hook_event_name": "UserPromptSubmit", "session_id": "sess-0b5c" }),
    );
    let agent = on_hook(agent, &start("sess-77aa"));
    let a = agent.clone().unwrap();
    assert_eq!(
        (a.session.as_str(), a.state),
        ("sess-77aa", AgentState::Idle)
    );
    // The old session ending later leaves the new one alone.
    let agent = on_hook(
        agent,
        &json!({ "hook_event_name": "SessionEnd", "session_id": "sess-0b5c" }),
    );
    assert_eq!(agent.as_ref().unwrap().session, "sess-77aa");
    // An id that could read as an option never becomes a session.
    assert_eq!(on_hook(None, &start("--resume")), None);
    // A relative transcript path is dropped.
    let e = json!({ "hook_event_name": "SessionStart", "session_id": "s1", "transcript_path": "x.jsonl" });
    assert_eq!(on_hook(None, &e).unwrap().transcript, None);
}
