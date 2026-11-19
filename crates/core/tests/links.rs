//! refine-context-links: cleaned, unique and named links in a side tab's context.

use terminescu_core::context::{self, ItemKind};
use terminescu_core::db::Db;
use terminescu_core::extract::{self, Found};
use terminescu_core::layout::Layout;
use terminescu_core::links::NameRank;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};

const PR: &str = "https://github.com/acme/shop-api/pull/918";
const ARTIFACT: &str = "https://claude.ai/code/artifact/0000-aaaa";

/// A database with side tab `s1` holding tabs `t1` and `t2`.
fn open() -> Db {
    let db = Db::open_in_memory().unwrap();
    let tab = |id: &str, pane: &str| Tab {
        accent: None,
        status: None,
        icon: None,
        id: id.into(),
        title: None,
        layout: Layout::pane(pane),
    };
    let workspace = Workspace {
        windows: vec![Window {
            id: "w1".into(),
            zoom_percent: None,
            side_tabs: vec![SideTab {
                folder: false,
                accent: None,
                name: None,
                status: None,
                icon: None,
                id: "s1".into(),
                title: None,
                tabs: vec![tab("t1", "p1"), tab("t2", "p2")],
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

fn rows(db: &Db) -> Vec<(String, String)> {
    context::items(db, "s1")
        .unwrap()
        .into_iter()
        .map(|i| (i.value, i.label))
        .collect()
}

#[test]
fn two_forms_of_one_pr_make_one_row() {
    let db = open();
    context::add_item(&db, "s1", ItemKind::Link, PR, "pasted", None).unwrap();
    let again = format!("{PR}/commits?utm_source=x");
    let (_, new) =
        context::add_named_item(&db, "s1", ItemKind::Link, &again, "pasted", None, None).unwrap();
    assert!(!new);
    assert_eq!(
        rows(&db),
        [(PR.to_string(), "PR #918 · shop-api".to_string())]
    );
}

#[test]
fn a_stronger_name_wins_and_the_same_rank_renames() {
    let db = open();
    let tool = Some(("Release checklist", NameRank::Tool));
    let (item, new) =
        context::add_named_item(&db, "s1", ItemKind::Link, ARTIFACT, "auto", None, tool).unwrap();
    assert!(new);
    assert_eq!(item.label, "Release checklist");

    // A reply's `[the page](url)` does not override the tool's name.
    let text = Some(("the page", NameRank::Text));
    context::add_named_item(&db, "s1", ItemKind::Link, ARTIFACT, "auto", None, text).unwrap();
    assert_eq!(rows(&db)[0].1, "Release checklist");

    // The same rank renames: a PR title that changed shows up.
    assert!(
        context::name_link(&db, "s1", ARTIFACT, "Release checklist v2", NameRank::Tool).unwrap()
    );
    assert_eq!(rows(&db)[0].1, "Release checklist v2");
    assert!(!context::name_link(&db, "s1", ARTIFACT, "here", NameRank::Tool).unwrap());
}

#[test]
fn a_reply_names_a_link_and_a_bare_mention_keeps_the_name() {
    let db = open();
    let reply = format!("Opened [Retry failed payments]({PR}/files). See {PR} too.");
    let found = extract::find(&reply);
    assert_eq!(
        found,
        [Found::link(
            PR,
            Some(("Retry failed payments".into(), NameRank::Text))
        )]
    );
    let r = extract::record_all(&db, "s1", None, &found).unwrap();
    assert_eq!((r.added, r.new_links.clone()), (1, vec![PR.to_string()]));
    assert_eq!(rows(&db)[0].1, "Retry failed payments");

    // Recording it again only renames when the name is better; here nothing changes.
    let r = extract::record_all(&db, "s1", None, &extract::find(&format!("see {PR}"))).unwrap();
    assert!(!r.changed());
    assert_eq!(rows(&db)[0].1, "Retry failed payments");

    // A filler word is no name.
    assert_eq!(
        extract::find(&format!("[here]({ARTIFACT})")),
        [Found::link(ARTIFACT, None)]
    );
}

#[test]
fn saved_links_are_cleaned_once_and_copies_keep_their_rows() {
    let db = open();
    let owner = db.local_owner().unwrap();
    let raw =
        |id: &str, value: &str, origin: Option<&str>, name: Option<&str>, rank: i64, pos: i64| {
            db.conn()
            .execute(
                "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source,
                   origin_tab_id, position, created_at, updated_at, name_rank)
                 VALUES (?1, ?2, 's1', 'link', ?3, ?4, 'auto', ?5, ?6, 0, 0, ?7)",
                rusqlite::params![id, owner, name, value, origin, pos, rank],
            )
            .unwrap();
        };
    raw("a", "https://example.test/a/", None, None, 0, 0);
    raw(
        "b",
        "https://example.test/a?utm_source=x",
        Some("t2"),
        Some("Audit guide"),
        1,
        1,
    );
    raw("c", "not a url", None, None, 0, 2);

    assert!(context::clean_saved_links(&db).unwrap() > 0);
    assert_eq!(
        rows(&db),
        [
            (
                "https://example.test/a".to_string(),
                "Audit guide".to_string()
            ),
            ("not a url".to_string(), "not a url".to_string()),
        ]
    );
    // Both copies stay, so the moved tab's copy still moves with t2.
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM context_items WHERE value = 'https://example.test/a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // Once: a later start changes nothing, even with a new raw row.
    raw("d", "https://example.test/b/", None, None, 0, 3);
    assert_eq!(context::clean_saved_links(&db).unwrap(), 0);
}
