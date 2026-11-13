//! Task 6.7: saved commands resolve `{{variables}}`, Run types Enter and Insert does not (D24).

use std::collections::BTreeMap;
use terminescu_core::commands::{self, NewCommand, Target, type_bytes};
use terminescu_core::context::{Mode, Variable};
use terminescu_core::db::Db;
use terminescu_core::layout::Layout;
use terminescu_core::persist::{Saved, save};
use terminescu_core::state::{SideTab, Tab, Window, Workspace};

fn var(name: &str, value: Option<&str>, mode: Mode) -> Variable {
    Variable {
        id: format!("v-{name}"),
        side_tab_id: "s1".into(),
        name: name.into(),
        value: value.map(Into::into),
        mode,
        source: Some("typed".into()),
        origin_tab_id: None,
        expires_at: None,
    }
}

fn db() -> Db {
    let db = Db::open_in_memory().unwrap();
    let side = |id: &str, tab: &str, pane: &str| SideTab {
        folder: false,
        status: None,
        name: None,
        accent: None,
        icon: None,
        id: id.into(),
        title: None,
        tabs: vec![Tab {
            accent: None,
            status: None,
            icon: None,
            id: tab.into(),
            title: None,
            layout: Layout::pane(pane),
        }],
    };
    let workspace = Workspace {
        windows: vec![Window {
            id: "w1".into(),
            side_tabs: vec![side("s1", "t1", "p1"), side("s2", "t2", "p2")],
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

#[test]
fn resolves_pr_api_and_a_secret_by_reference() {
    let vars = [
        var(
            "pr_api",
            Some("https://github.com/acme/api/pull/918"),
            Mode::Saved,
        ),
        var("API_TOKEN", None, Mode::Secret),
    ];
    let r = commands::resolve(
        "gh pr checks {{pr_api}} && curl -H \"Authorization: {{ API_TOKEN }}\" {{host}}",
        &vars,
        &BTreeMap::new(),
    );
    assert_eq!(
        r.text,
        "gh pr checks https://github.com/acme/api/pull/918 && curl -H \"Authorization: $API_TOKEN\" {{host}}"
    );
    assert_eq!(r.missing, ["host"]);
    assert!(r.unsafe_for_run.is_empty());

    let extra = BTreeMap::from([("host".to_string(), "example.com".to_string())]);
    let r = commands::resolve("curl {{host}}", &vars, &extra);
    assert_eq!((r.text.as_str(), r.missing.len()), ("curl example.com", 0));

    // A value with a line break would run more than one command on Enter.
    let vars = [var("note", Some("a\nrm -rf ~"), Mode::Session)];
    let r = commands::resolve("echo {{note}}", &vars, &BTreeMap::new());
    assert_eq!(r.unsafe_for_run, ["note"]);
}

#[test]
fn run_writes_one_cr_and_insert_none() {
    assert_eq!(type_bytes("gh pr checks 918", true), b"gh pr checks 918\r");
    assert_eq!(type_bytes("gh pr checks 918", false), b"gh pr checks 918");
    // Two lines go in as one bracketed paste, so the first never runs alone.
    assert_eq!(
        type_bytes("cd api\r\npnpm dev\x1b[201~", false),
        b"\x1b[200~cd api\npnpm dev\x1b[201~"
    );
    assert!(!type_bytes("a\rb", false).contains(&b'\r'));
    // A tab would complete in the shell, so one line with a tab goes bracketed too.
    assert_eq!(type_bytes("a\tb", false), b"\x1b[200~a\tb\x1b[201~");
}

#[test]
fn lists_the_side_tabs_own_commands_before_the_global_ones() {
    let db = db();
    let new = |side: Option<&str>, name: &str, body: &str| NewCommand {
        side_tab_id: side.map(Into::into),
        name: name.into(),
        body: body.into(),
        target: Target::NewTab,
        launcher: false,
    };
    let global = commands::create(&db, &new(None, "status", "git status")).unwrap();
    let checks =
        commands::create(&db, &new(Some("s1"), "checks", "gh pr checks {{pr_api}}\n")).unwrap();
    commands::create(&db, &new(Some("s2"), "other", "ls")).unwrap();
    assert_eq!(
        checks.body, "gh pr checks {{pr_api}}",
        "a trailing line break is dropped"
    );
    let names = |side| {
        commands::list(&db, side)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(Some("s1")), ["checks", "status"]);
    assert_eq!(names(None), ["status"]);
    // Settings › Commands lists every one, global first.
    let all: Vec<_> = commands::all(&db)
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(all, ["status", "checks", "other"]);

    let mut edit = new(None, "git status", "git status -sb");
    edit.target = Target::Focused;
    let edited = commands::update(&db, &global.id, &edit).unwrap();
    assert_eq!(
        (edited.body.as_str(), edited.target),
        ("git status -sb", Target::Focused)
    );
    assert!(commands::create(&db, &new(None, " ", "ls")).is_err());
    assert!(commands::create(&db, &new(None, "x", " \n")).is_err());
    commands::delete(&db, &global.id).unwrap();
    assert_eq!(names(Some("s1")), ["checks"]);
}

#[test]
fn a_split_paste_marker_cannot_end_the_paste() {
    let evil = "echo hi\x1b[2\x1b[201~01~\nrm -rf ~/work\n";
    let bytes = String::from_utf8(type_bytes(evil, false)).unwrap();
    assert_eq!(bytes, "\x1b[200~echo hi\nrm -rf ~/work\n\x1b[201~");
    assert_eq!(
        commands::typeable("a\x07b\u{9b}c\td\r\ne"),
        "abc\td\ne",
        "C0, C1 and DEL go; tab and line breaks stay"
    );
}

#[test]
fn a_global_command_cannot_become_a_launcher_through_update() {
    let db = db();
    let global = commands::create(
        &db,
        &NewCommand {
            side_tab_id: None,
            name: "status".into(),
            body: "git status".into(),
            target: Target::Focused,
            launcher: false,
        },
    )
    .unwrap();
    // A side tab id in the update does not move the command, so the flag is refused.
    let as_launcher = NewCommand {
        side_tab_id: Some("s1".into()),
        name: "status".into(),
        body: "git status".into(),
        target: Target::Focused,
        launcher: true,
    };
    assert!(matches!(
        commands::update(&db, &global.id, &as_launcher),
        Err(commands::CommandStoreError::GlobalLauncher)
    ));
}
