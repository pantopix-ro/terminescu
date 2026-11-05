//! Task 8.2: merging side tabs (D5). Clashes, renames, concatenation, undo, and items that
//! follow a tab moved alone. Daemon variables need the pty daemon and are covered in ptyd.

use std::collections::BTreeMap;
use terminescu_core::commands::{self, NewCommand};
use terminescu_core::context::{self, ItemKind, Pin};
use terminescu_core::db::Db;
use terminescu_core::files;
use terminescu_core::layout::Layout;
use terminescu_core::merge::{self, Resolution};
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};
use terminescu_core::vars;

fn tab(id: &str, pane: &str) -> Tab {
    Tab {
        status: None,
        id: id.into(),
        title: None,
        layout: Layout::pane(pane),
    }
}

fn side(id: &str, title: &str, tabs: Vec<Tab>) -> SideTab {
    SideTab {
        status: None,
        id: id.into(),
        title: Some(title.into()),
        tabs,
    }
}

fn workspace(sides: Vec<SideTab>) -> Workspace {
    Workspace {
        windows: vec![Window {
            id: "w1".into(),
            side_tabs: sides,
        }],
    }
}

fn store(db: &Db, ws: Workspace) {
    save(
        db,
        &Saved {
            workspace: ws,
            ..Default::default()
        },
    )
    .unwrap();
}

/// PROJ-477 (s1, tab t1) and PROJ-482 (s2, tab t2), each with a branch of its own.
fn two() -> Db {
    let db = Db::open_in_memory().unwrap();
    store(
        &db,
        workspace(vec![
            side("s1", "PROJ-477", vec![tab("t1", "p1")]),
            side("s2", "PROJ-482", vec![tab("t2", "p2")]),
        ]),
    );
    let var = |s: &str, n: &str, v: &str| {
        context::put_saved(&db, s, n, v, "typed", None, false).unwrap();
    };
    var("s2", "branch", "feature/PROJ-482");
    var("s2", "repo", "acme/api");
    var("s1", "branch", "feature/PROJ-477");
    var("s1", "repo", "acme/api");
    var("s1", "pr_api", "https://example.com/acme/api/pull/12");
    let link = |s: &str, v: &str| {
        context::add_item(&db, s, ItemKind::Link, v, "typed", None).unwrap();
    };
    link("s2", "https://example.com/docs/audit");
    link("s1", "https://example.com/docs/audit");
    link("s1", "https://example.com/acme/api/pull/12");
    context::add_item(
        &db,
        "s1",
        ItemKind::Text,
        "export runs nightly",
        "pasted",
        None,
    )
    .unwrap();
    context::add_pin(
        &db,
        &Pin {
            id: String::new(),
            side_tab_id: "s1".into(),
            origin_tab_id: Some("t1".into()),
            session: None,
            entry_id: None,
            pane_id: Some("p1".into()),
            row_start: Some(1),
            row_end: Some(2),
            exact: "cargo test".into(),
            prefix: None,
            suffix: None,
            start: None,
            end: None,
            created_at: 0,
        },
    )
    .unwrap();
    commands::create(
        &db,
        &NewCommand {
            side_tab_id: Some("s1".into()),
            name: "Tests".into(),
            body: "cargo test".into(),
            target: Default::default(),
            launcher: false,
        },
    )
    .unwrap();
    db
}

fn names(db: &Db, side: &str) -> Vec<(String, String)> {
    vars::list(db, None, side)
        .unwrap()
        .into_iter()
        .map(|v| (v.name, v.value.unwrap_or_default()))
        .collect()
}

fn values(db: &Db, side: &str) -> Vec<String> {
    context::items(db, side)
        .unwrap()
        .into_iter()
        .map(|i| i.value)
        .collect()
}

#[test]
fn a_clash_is_a_name_with_two_values_and_suggests_the_ticket_number() {
    let db = two();
    let clashes = merge::preview(&db, None, "s1", "s2", Some("PROJ-477")).unwrap();
    assert_eq!(clashes.len(), 1, "repo has one value in both, so no clash");
    assert_eq!(clashes[0].name, "branch");
    assert_eq!(clashes[0].from_value.as_deref(), Some("feature/PROJ-477"));
    assert_eq!(clashes[0].into_value.as_deref(), Some("feature/PROJ-482"));
    assert_eq!(clashes[0].rename, "branch_477");

    let taken = ["branch_477".to_string()].into_iter().collect();
    assert_eq!(
        merge::suggest("branch", Some("PROJ-477"), &taken),
        "branch_477_2"
    );
    assert_eq!(
        merge::suggest("branch", Some("audit"), &Default::default()),
        "branch_2"
    );
}

#[test]
fn keep_both_renames_the_source_and_lists_concatenate() {
    let db = two();
    let undo = merge::merge(
        &db,
        None,
        "s1",
        "s2",
        Some("PROJ-477"),
        &Resolution {
            keep_both: true,
            renames: BTreeMap::new(),
        },
    )
    .unwrap();
    assert_eq!(undo.renames, [("branch".into(), "branch_477".into())]);
    assert_eq!(
        names(&db, "s2"),
        [
            ("branch".into(), "feature/PROJ-482".into()),
            ("repo".into(), "acme/api".into()),
            ("branch_477".into(), "feature/PROJ-477".into()),
            (
                "pr_api".into(),
                "https://example.com/acme/api/pull/12".into()
            ),
        ]
    );
    // The target's items first, then the source's; the link both had shows once.
    assert_eq!(
        values(&db, "s2"),
        [
            "https://example.com/docs/audit",
            "https://example.com/acme/api/pull/12",
            "export runs nightly",
        ]
    );
    assert_eq!(context::pins(&db, "s2").unwrap().len(), 1);
    assert!(names(&db, "s1").is_empty() && values(&db, "s1").is_empty());
    let local = |s: &str| {
        commands::list(&db, Some(s))
            .unwrap()
            .into_iter()
            .filter(|c| c.side_tab_id.as_deref() == Some(s))
            .count()
    };
    assert_eq!((local("s1"), local("s2")), (0, 1));
}

#[test]
fn undo_of_keep_both_gives_the_renamed_variable_its_name_back() {
    let db = two();
    let before = (names(&db, "s1"), names(&db, "s2"));
    let keep_both = Resolution {
        keep_both: true,
        renames: BTreeMap::new(),
    };
    let undo = merge::merge(&db, None, "s1", "s2", Some("PROJ-477"), &keep_both).unwrap();
    merge::undo(&db, None, &undo).unwrap();
    assert_eq!((names(&db, "s1"), names(&db, "s2")), before);
}

#[test]
fn keep_target_drops_the_source_value_and_undo_brings_everything_back() {
    let db = two();
    let before = (
        names(&db, "s1"),
        values(&db, "s1"),
        names(&db, "s2"),
        values(&db, "s2"),
    );
    let undo = merge::merge(&db, None, "s1", "s2", None, &Resolution::default()).unwrap();
    assert!(undo.renames.is_empty());
    assert_eq!(
        names(&db, "s2")
            .into_iter()
            .filter(|(n, _)| n.starts_with("branch"))
            .collect::<Vec<_>>(),
        [("branch".to_string(), "feature/PROJ-482".to_string())]
    );
    merge::undo(&db, None, &undo).unwrap();
    let after = (
        names(&db, "s1"),
        values(&db, "s1"),
        names(&db, "s2"),
        values(&db, "s2"),
    );
    assert_eq!(before, after);
    assert_eq!(context::pins(&db, "s1").unwrap().len(), 1);
}

#[test]
fn a_typed_rename_must_be_a_free_variable_name() {
    let db = two();
    let bad = |to: &str| {
        merge::merge(
            &db,
            None,
            "s1",
            "s2",
            None,
            &Resolution {
                keep_both: true,
                renames: [("branch".to_string(), to.to_string())].into(),
            },
        )
        .is_err()
    };
    assert!(bad("1branch"));
    assert!(bad("repo"));
    assert_eq!(names(&db, "s1").len(), 3, "a refused merge changes nothing");
}

#[test]
fn a_tab_moved_alone_takes_what_it_made() {
    let db = two();
    context::add_item(
        &db,
        "s1",
        ItemKind::Link,
        "https://example.com/acme/api/pull/31",
        "auto",
        Some("t1"),
    )
    .unwrap();
    context::put_saved(
        &db,
        "s1",
        "ticket",
        "PROJ-477",
        "extracted",
        Some("t1"),
        false,
    )
    .unwrap();
    context::put_saved(&db, "s1", "note_by_hand", "x", "typed", Some("t1"), false).unwrap();
    let touch = files::Touch {
        path: "/Users/owner/acme-api/src/audit_export.rs".into(),
        change: files::Change::Modified,
        session: None,
        cwd: Some("/Users/owner/acme-api".into()),
    };
    files::record(&db, "s1", Some("t1"), &touch).unwrap();
    // t1 leaves s1 for s2; s1 keeps a second tab so it stays open.
    store(
        &db,
        workspace(vec![
            side("s1", "PROJ-477", vec![tab("t3", "p3")]),
            side("s2", "PROJ-482", vec![tab("t2", "p2"), tab("t1", "p1")]),
        ]),
    );
    let s2: Vec<String> = names(&db, "s2").into_iter().map(|(n, _)| n).collect();
    assert!(s2.contains(&"ticket".to_string()));
    assert!(!s2.contains(&"note_by_hand".to_string()), "typed stays");
    assert!(values(&db, "s2").contains(&"https://example.com/acme/api/pull/31".to_string()));
    assert_eq!(context::pins(&db, "s2").unwrap().len(), 1);
    assert_eq!(files::list(&db, "s2").unwrap().len(), 1);
    assert!(files::list(&db, "s1").unwrap().is_empty());
}
