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

fn link(event: &Value) -> Option<(String, Option<String>)> {
    events(event).into_iter().find_map(|e| match e {
        AgentEvent::Link { url, name } => Some((url, name)),
        _ => None,
    })
}

const ARTIFACT: &str = "https://claude.ai/code/artifact/3f2a9c1e-7b44-4d0a-9e1f-0c5d2b8a6e71";

/// refine-context-links D7. The response's shape is not fixed: the URL is the first artifact
/// URL in any string of it, the name its `title` key, the input's, or the page's `<title>`.
#[test]
fn an_artifact_call_names_its_link() {
    let mut event: Value =
        serde_json::from_str(include_str!("fixtures/hooks/post-tool-use-artifact.json")).unwrap();
    // The fixture's file is not on disk and the response has no `title` key: no name.
    assert_eq!(link(&event), Some((ARTIFACT.into(), None)));

    event["tool_response"] = json!({ "url": ARTIFACT, "title": "Release checklist" });
    assert_eq!(
        link(&event),
        Some((ARTIFACT.into(), Some("Release checklist".into())))
    );

    event["tool_response"] = json!({ "url": ARTIFACT });
    event["tool_input"]["title"] = json!("Checklist from input");
    assert_eq!(
        link(&event),
        Some((ARTIFACT.into(), Some("Checklist from input".into())))
    );

    // Another tool, or no URL, makes no link.
    assert_eq!(
        link(&post("Write", json!({}), json!({ "url": ARTIFACT }))),
        None
    );
    assert_eq!(link(&post("Artifact", json!({}), json!("done"))), None);

    // Only a publish makes a link: a list, read or delete names other pages, and an asset
    // upload is not the page.
    for input in [
        json!({ "action": "list" }),
        json!({ "action": "read", "url": ARTIFACT }),
        json!({ "action": "delete", "url": ARTIFACT }),
        json!({ "url": ARTIFACT, "file_path": "/x/a.png", "asset": true }),
    ] {
        let e = post(
            "Artifact",
            input.clone(),
            json!({ "url": ARTIFACT, "title": "Other" }),
        );
        assert_eq!(link(&e), None, "{input}");
    }
    // A republish names the page by its input `url`, before any other URL in the response.
    let other = "https://claude.ai/code/artifact/0000-bbbb";
    let e = post(
        "Artifact",
        json!({ "url": ARTIFACT, "file_path": "/x/page.html" }),
        json!(format!("See {other}; updated {ARTIFACT}")),
    );
    assert_eq!(link(&e), Some((ARTIFACT.into(), None)));
}

#[test]
fn an_artifact_page_title_comes_from_its_file_in_the_session_folder() {
    let dir = std::env::temp_dir().join(format!("terminescu-artifact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("page.html");
    std::fs::write(
        &page,
        "<meta charset=utf-8><TITLE>\n  Q3 &amp; Q4 plan </TITLE><p>body",
    )
    .unwrap();
    let event = post(
        "Artifact",
        json!({ "file_path": page.display().to_string() }),
        json!(format!("Published {ARTIFACT}")),
    );
    assert_eq!(
        link(&event),
        Some((ARTIFACT.into(), Some("Q3 & Q4 plan".into())))
    );

    // A relative path, or a file that is not HTML, is not read.
    let txt = dir.join("page.txt");
    std::fs::write(&txt, "<title>No</title>").unwrap();
    for path in ["page.html".to_string(), txt.display().to_string()] {
        let event = post("Artifact", json!({ "file_path": path }), json!(ARTIFACT));
        assert_eq!(link(&event), Some((ARTIFACT.into(), None)));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_artifact_page_outside_the_allowed_folders_is_not_read() {
    use terminescu_plugin_claude::agent::html_title;
    // `/` is neither the session folder nor a temp folder.
    assert_eq!(
        html_title(
            std::path::Path::new("/etc/hosts.html"),
            Some(std::path::Path::new("/Users/owner/x"))
        ),
        None
    );
}
