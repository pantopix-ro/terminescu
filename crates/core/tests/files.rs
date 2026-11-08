//! Task 7.10: the Files section takes `PostToolUse` events for file-editing tools, one entry
//! per path (D30).

use serde_json::json;
use terminescu_core::db::Db;
use terminescu_core::files::{self, Change};
use terminescu_core::layout::Layout;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};

fn db() -> Db {
    let db = Db::open_in_memory().unwrap();
    let workspace = Workspace {
        windows: vec![Window {
            id: "w1".into(),
            side_tabs: vec![SideTab {
                status: None,
                name: None,
                accent: None,
                icon: None,
                id: "s1".into(),
                title: None,
                tabs: vec![Tab {
                    accent: None,
                    status: None,
                    icon: None,
                    id: "t1".into(),
                    title: None,
                    layout: Layout::pane("p1"),
                }],
            }],
        }],
    };
    save(
        &db,
        &Saved {
            workspace,
            ..Default::default()
        },
    )
    .unwrap();
    db
}

fn post(tool: &str, input: serde_json::Value, response: serde_json::Value) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "session_id": "sess-0b5c",
        "cwd": "/Users/owner/acme-api",
        "tool_name": tool,
        "tool_input": input,
        "tool_response": response,
    })
}

#[test]
fn two_events_for_one_path_make_one_entry_that_stays_added() {
    let db = db();
    let write = post(
        "Write",
        json!({"file_path": "/Users/owner/acme-api/src/audit_export.rs", "content": "fn main() {}"}),
        json!({"type": "create"}),
    );
    let edit = post(
        "Edit",
        json!({"file_path": "/Users/owner/acme-api/src/audit_export.rs", "old_string": "a", "new_string": "b"}),
        json!({}),
    );
    for e in [&write, &edit] {
        files::record(&db, "s1", Some("t1"), &files::touched(e).unwrap()).unwrap();
    }
    let list = files::list(&db, "s1").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].shown, "src/audit_export.rs");
    assert_eq!(list[0].change, Change::Added);
    assert_eq!(list[0].origin_tab_id.as_deref(), Some("t1"));

    let readme = post(
        "MultiEdit",
        json!({"file_path": "README.md", "edits": []}),
        json!({}),
    );
    let t = files::touched(&readme).unwrap();
    assert_eq!(t.path, "/Users/owner/acme-api/README.md");
    files::record(&db, "s1", None, &t).unwrap();
    let list = files::list(&db, "s1").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        list.iter().find(|f| f.shown == "README.md").unwrap().change,
        Change::Modified
    );

    assert_eq!(
        files::remove(&db, &list[0].id).unwrap().as_deref(),
        Some("s1")
    );
    assert_eq!(files::list(&db, "s1").unwrap().len(), 1);
}

#[test]
fn only_file_editing_tools_after_they_ran_count() {
    let nb = post(
        "NotebookEdit",
        json!({"notebook_path": "/Users/owner/acme-api/a.ipynb"}),
        json!({}),
    );
    assert_eq!(files::touched(&nb).unwrap().change, Change::Modified);
    let over = post(
        "Write",
        json!({"file_path": "/tmp/x.md"}),
        json!({"type": "update"}),
    );
    assert_eq!(files::touched(&over).unwrap().change, Change::Modified);
    assert!(files::touched(&post("Read", json!({"file_path": "/tmp/x.md"}), json!({}))).is_none());
    assert!(files::touched(&post("Bash", json!({"command": "ls"}), json!({}))).is_none());
    let mut pre = post("Write", json!({"file_path": "/tmp/x.md"}), json!({}));
    pre["hook_event_name"] = json!("PreToolUse");
    assert!(files::touched(&pre).is_none());
    // A relative path with no cwd has nothing to resolve against.
    let mut bare = post("Edit", json!({"file_path": "x.md"}), json!({}));
    bare.as_object_mut().unwrap().remove("cwd");
    assert!(files::touched(&bare).is_none());
}
