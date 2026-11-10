//! Task 6.10: Settings › Keys saves the person's chords and refuses clashes (D17).

use terminescu_core::db::Db;
use terminescu_core::keys::{self, Bindings, KeysError};

fn defaults() -> Vec<(String, String)> {
    [
        ("new_tab", "Cmd+T"),
        ("open_file", "Cmd+O"),
        ("split_right", "Cmd+D"),
    ]
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .to_vec()
}

fn one(id: &str, accel: &str) -> Bindings {
    Bindings::from([(id.to_string(), accel.to_string())])
}

#[test]
fn rebinds_open_file_and_drops_a_binding_equal_to_the_default() {
    let db = Db::open_in_memory().unwrap();
    keys::set(&db, &defaults(), one("open_file", "Cmd+P")).unwrap();
    assert_eq!(keys::get(&db).unwrap(), one("open_file", "Cmd+P"));
    let applied = keys::apply(&defaults(), &keys::get(&db).unwrap())
        .into_iter()
        .map(|(id, a)| format!("{id}={a}"))
        .collect::<Vec<_>>();
    assert_eq!(
        applied,
        ["new_tab=Cmd+T", "open_file=Cmd+P", "split_right=Cmd+D"]
    );
    keys::set(&db, &defaults(), one("open_file", "cmd+o")).unwrap();
    assert!(keys::get(&db).unwrap().is_empty());
}

#[test]
fn refuses_a_clash_a_chord_without_cmd_a_reserved_one_and_an_unknown_action() {
    let db = Db::open_in_memory().unwrap();
    let err = |b| keys::set(&db, &defaults(), b).unwrap_err();
    // ⇧⌘T is not ⌘T.
    keys::set(&db, &defaults(), one("open_file", "Shift+Cmd+t")).unwrap();
    assert!(matches!(
        err(one("open_file", "Cmd+T")),
        KeysError::Clash(..)
    ));
    assert!(matches!(
        err(one("open_file", "Ctrl+P")),
        KeysError::NoCmd(_)
    ));
    assert!(matches!(
        err(one("open_file", "Cmd+C")),
        KeysError::Reserved(_)
    ));
    assert!(matches!(
        err(one("open_file", "Cmd++")),
        KeysError::NoKey(_)
    ));
    assert!(matches!(
        err(one("open_file", "Cmd+Shift")),
        KeysError::NoKey(_)
    ));
    assert!(matches!(
        err(one("nope", "Cmd+P")),
        KeysError::UnknownAction(_)
    ));
    // Swapping two chords in one save is fine.
    let swap = Bindings::from([
        ("new_tab".to_string(), "Cmd+D".to_string()),
        ("split_right".to_string(), "Cmd+T".to_string()),
    ]);
    keys::set(&db, &defaults(), swap.clone()).unwrap();
    assert_eq!(keys::get(&db).unwrap(), swap);
}

#[test]
fn normal_ignores_order_and_case() {
    assert_eq!(keys::normal("Shift+Cmd+p"), keys::normal("Cmd+Shift+P"));
    assert_ne!(keys::normal("Cmd+P"), keys::normal("Cmd+Shift+P"));
}

#[test]
fn plugin_chords_and_off_chords_save_and_clash_with_app_chords() {
    let db = Db::open_in_memory().unwrap();
    // A plugin id keeps its chord: the core does not know the plugin's default.
    keys::set(&db, &defaults(), one("git.newWorktreeRow", "Cmd+Alt+W")).unwrap();
    assert_eq!(
        keys::get(&db).unwrap(),
        one("git.newWorktreeRow", "Cmd+Alt+W")
    );
    // Off is an empty chord, for app and plugin actions alike, and clashes with nothing.
    let off = Bindings::from([
        ("new_tab".to_string(), "".to_string()),
        ("git.newWorktreeRow".to_string(), "".to_string()),
        ("claude.resume".to_string(), "".to_string()),
    ]);
    assert_eq!(keys::set(&db, &defaults(), off.clone()).unwrap(), off);
    // A freed app chord can go to a plugin action.
    let moved = Bindings::from([
        ("new_tab".to_string(), "".to_string()),
        ("git.newWorktreeRow".to_string(), "Cmd+T".to_string()),
    ]);
    keys::set(&db, &defaults(), moved).unwrap();
    let err = |b| keys::set(&db, &defaults(), b).unwrap_err();
    assert!(matches!(
        err(one("git.newWorktreeRow", "Cmd+D")),
        KeysError::Clash(..)
    ));
    assert!(matches!(
        err(one("git.newWorktreeRow", "Cmd+Q")),
        KeysError::Reserved(_)
    ));
    assert!(matches!(
        err(one("git.a.b", "Cmd+P")),
        KeysError::UnknownAction(_)
    ));
}

#[test]
fn a_saved_chord_that_no_longer_passes_drops_alone() {
    let db = Db::open_in_memory().unwrap();
    keys::set(
        &db,
        &defaults(),
        Bindings::from([
            ("open_file".to_string(), "Cmd+P".to_string()),
            ("split_right".to_string(), "Cmd+Shift+D".to_string()),
        ]),
    )
    .unwrap();
    // A later build gives a new action ⌘P and drops split_right.
    let later: Vec<(String, String)> = [
        ("new_tab", "Cmd+T"),
        ("open_file", "Cmd+O"),
        ("palette", "Cmd+P"),
    ]
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .to_vec();
    assert!(keys::check(&later, keys::get(&db).unwrap()).is_err());
    assert!(keys::read(&db, &later).unwrap().is_empty());
    // An entry that still passes stays.
    let kept: Vec<(String, String)> = [("new_tab", "Cmd+T"), ("open_file", "Cmd+O")]
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .to_vec();
    assert_eq!(keys::read(&db, &kept).unwrap(), one("open_file", "Cmd+P"));
}
