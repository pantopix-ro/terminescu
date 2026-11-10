//! Task 7.10: the Files section keeps one entry per path an agent touched (D30). Which hook
//! events touch a file is the adapter's to say (add-plugin-architecture 3.2).

use terminescu_core::db::Db;
use terminescu_core::files::{self, Change, Touch};
use terminescu_core::layout::Layout;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};
use terminescu_plugin_api::AgentEvent;

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

fn touch(path: &str, created: bool) -> Touch {
    Touch::of(AgentEvent::Touched {
        path: path.into(),
        created,
        session: Some("sess-0b5c".into()),
        cwd: Some("/Users/owner/acme-api".into()),
    })
    .unwrap()
}

#[test]
fn two_events_for_one_path_make_one_entry_that_stays_added() {
    let db = db();
    for t in [
        touch("/Users/owner/acme-api/src/audit_export.rs", true),
        touch("/Users/owner/acme-api/src/./x/../audit_export.rs", false),
    ] {
        files::record(&db, "s1", Some("t1"), &t).unwrap();
    }
    let list = files::list(&db, "s1").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].shown, "src/audit_export.rs");
    assert_eq!(list[0].change, Change::Added);
    assert_eq!(list[0].origin_tab_id.as_deref(), Some("t1"));

    files::record(
        &db,
        "s1",
        None,
        &touch("/Users/owner/acme-api/README.md", false),
    )
    .unwrap();
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
fn only_an_absolute_touched_path_counts() {
    let rel = AgentEvent::Touched {
        path: "x.md".into(),
        created: false,
        session: None,
        cwd: None,
    };
    assert!(Touch::of(rel).is_none());
    assert!(Touch::of(AgentEvent::Reply("done".into())).is_none());
}
