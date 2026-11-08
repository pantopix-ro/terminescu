//! Task 8.3: extraction rules on assistant entries and terminal output (D13).

use terminescu_core::context::{self, ItemKind};
use terminescu_core::db::Db;
use terminescu_core::extract::{Found, Scanner, find, record, strip_ansi};
use terminescu_core::layout::Layout;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};

fn link(v: &str) -> Found {
    Found {
        kind: ItemKind::Link,
        value: v.into(),
    }
}

fn text(v: &str) -> Found {
    Found {
        kind: ItemKind::Text,
        value: v.into(),
    }
}

const ARTIFACT: &str = "https://claude.ai/artifact/3f2a9c1e-7b44-4d0a-9e1f-0c5d2b8a6e71";

#[test]
fn an_artifact_url_twice_in_an_entry_yields_one_auto_link() {
    let entry = include_str!("fixtures/assistant-artifact-twice.txt");
    let found = find(entry);
    assert_eq!(
        found,
        vec![
            link(ARTIFACT),
            link("https://github.com/acme/api/pull/918"),
            text("feature/audit-export"),
        ]
    );

    let db = Db::open_in_memory().unwrap();
    let side = side_tab(&db);
    assert_eq!(record(&db, &side, None, &found).unwrap(), 3);
    // The same entry read again adds nothing.
    assert_eq!(record(&db, &side, None, &find(entry)).unwrap(), 0);
    let items = context::items(&db, &side).unwrap();
    assert_eq!(items.iter().filter(|i| i.value == ARTIFACT).count(), 1);
    assert!(items.iter().all(|i| i.source.as_deref() == Some("auto")));
}

#[test]
fn branch_names_come_from_git_wording_and_skip_the_usual_ones() {
    let out = "On branch fix-login\nYour branch is up to date with 'origin/main'.\n\
               Switched to a new branch 'feature/audit-export'\n\
               $ git checkout -b spike\n * [new branch]      release/2.4 -> release/2.4\n\
               the branch protection rules\n";
    assert_eq!(
        find(out),
        vec![
            text("fix-login"),
            text("feature/audit-export"),
            text("release/2.4"),
            text("spike"),
        ]
    );
    assert!(find("On branch main\nbranch 'develop'").is_empty());
}

#[test]
fn links_must_be_prs_or_artifacts() {
    assert!(find("see https://github.com/acme/api/issues/12 and https://example.com/x").is_empty());
    assert_eq!(
        find("(https://github.com/acme/api/pull/77#discussion_r1)."),
        vec![link("https://github.com/acme/api/pull/77")]
    );
}

#[test]
fn the_scanner_reads_whole_lines_through_colour_codes_and_reports_a_match_once() {
    let mut s = Scanner::default();
    assert!(s.feed(b"\x1b[32mhttps://github.com/acme/api/pu").is_empty());
    assert_eq!(
        s.feed(b"ll/918\x1b[0m\r\n"),
        vec![link("https://github.com/acme/api/pull/918")]
    );
    assert!(s.feed(b"https://github.com/acme/api/pull/918\n").is_empty());
    assert_eq!(
        strip_ansi(b"\x1b]0;title\x07ok\x1b[1;31m!"),
        "ok!".to_string()
    );
}

fn side_tab(db: &Db) -> String {
    let workspace = Workspace {
        windows: vec![Window {
            id: "w1".into(),
            side_tabs: vec![SideTab {
                accent: None,
                name: None,
                status: None,
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
    let saved = Saved {
        workspace,
        ..Default::default()
    };
    save(db, &saved).unwrap();
    "s1".into()
}
