//! Settings › Keys (D17): the chords the person changed, by action id, and the checks that
//! keep each one a `⌘` chord no other action or macOS uses. The same checks run in the
//! window (`packages/host/src/keys.ts`) so a clash shows before it is saved.

use crate::db::Db;
use std::collections::BTreeMap;

const KEY: &str = "keys.bindings";

/// Chords macOS and the Edit menu keep, as in `RESERVED` in keys.ts.
const RESERVED: &[&str] = &[
    "Cmd+Q",
    "Cmd+H",
    "Cmd+Alt+H",
    "Cmd+M",
    "Ctrl+Cmd+F",
    "Cmd+Z",
    "Cmd+Shift+Z",
    "Cmd+X",
    "Cmd+C",
    "Cmd+V",
    "Cmd+A",
    "Cmd+Tab",
    "Cmd+Space",
];

pub type Bindings = BTreeMap<String, String>;

#[derive(Debug, thiserror::Error)]
pub enum KeysError {
    #[error("no action {0}")]
    UnknownAction(String),
    #[error("{0} names no key: use a word such as Equal for the + key")]
    NoKey(String),
    #[error("{0} needs ⌘, so plain keys stay with the terminal")]
    NoCmd(String),
    #[error("{0} is kept by macOS")]
    Reserved(String),
    #[error("{0} is the chord of both {1} and {2}")]
    Clash(String, String, String),
    #[error(transparent)]
    Db(#[from] crate::db::DbError),
}

/// A chord in one spelling: modifiers in a fixed order, the key in lower case.
pub fn normal(accel: &str) -> String {
    let (mut ctrl, mut alt, mut shift, mut cmd) = (false, false, false, false);
    let mut key = String::new();
    for part in accel.split('+').map(|p| p.trim().to_ascii_lowercase()) {
        match part.as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "cmd" | "command" | "super" | "cmdorctrl" | "commandorcontrol" => cmd = true,
            _ => key = part,
        }
    }
    let mods = [(ctrl, "ctrl"), (alt, "alt"), (shift, "shift"), (cmd, "cmd")];
    let mut out: Vec<&str> = mods.iter().filter(|(on, _)| *on).map(|(_, m)| *m).collect();
    out.push(&key);
    out.join("+")
}

/// The saved chords, unchecked.
pub fn get(db: &Db) -> crate::db::Result<Bindings> {
    Ok(crate::settings::read(db, KEY)?.unwrap_or_default())
}

/// The saved chords as they stand against `defaults`: an entry that no longer passes the checks
/// (an action gone, a default moved onto it) drops alone, and the rest stay (D3).
pub fn read(db: &Db, defaults: &[(String, String)]) -> crate::db::Result<Bindings> {
    let mut kept = Bindings::new();
    for (id, accel) in get(db)? {
        let mut next = kept.clone();
        next.insert(id, accel);
        if let Ok(ok) = check(defaults, next) {
            kept = ok;
        }
    }
    Ok(kept)
}

/// Checks `bindings` against the default chords (id, accel) and saves them. A binding equal to
/// the default is dropped, so a reset leaves no row behind.
pub fn set(
    db: &Db,
    defaults: &[(String, String)],
    bindings: Bindings,
) -> Result<Bindings, KeysError> {
    let kept = check(defaults, bindings)?;
    crate::settings::write(db, KEY, &kept)?;
    Ok(kept)
}

/// A plugin action's id, `plugin.action`: the core does not know its default (D1).
fn plugin_id(id: &str) -> bool {
    id.split_once('.')
        .is_some_and(|(p, a)| !p.is_empty() && !a.is_empty() && !a.contains('.'))
}

/// The chords with `bindings` applied, after the checks. An app binding equal to its default
/// drops; a plugin binding stays, since only the plugin knows its default. An empty chord is
/// off (D2) and clashes with nothing.
pub fn check(defaults: &[(String, String)], bindings: Bindings) -> Result<Bindings, KeysError> {
    let mut kept = Bindings::new();
    for (id, accel) in bindings {
        match defaults.iter().find(|(d, _)| *d == id) {
            Some((_, default)) => {
                if accel.trim().is_empty() || normal(&accel) != normal(default) {
                    kept.insert(id, accel.trim().to_string());
                }
            }
            None if plugin_id(&id) => {
                kept.insert(id, accel.trim().to_string());
            }
            None => return Err(KeysError::UnknownAction(id)),
        }
    }
    let plugins = kept.iter().filter(|(id, _)| plugin_id(id));
    let all = defaults
        .iter()
        .map(|(id, d)| (id, kept.get(id).unwrap_or(d)))
        .chain(plugins);
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (id, accel) in all {
        if accel.is_empty() {
            continue;
        }
        let n = normal(accel);
        // `Cmd++` splits into empty parts, and `Cmd+Shift` has no key at all.
        let key = n.rsplit('+').next().unwrap_or_default();
        if key.is_empty() || ["ctrl", "alt", "shift", "cmd"].contains(&key) {
            return Err(KeysError::NoKey(accel.clone()));
        }
        if !n.split('+').any(|m| m == "cmd") {
            return Err(KeysError::NoCmd(accel.clone()));
        }
        if kept.contains_key(id) && RESERVED.iter().any(|r| normal(r) == n) {
            return Err(KeysError::Reserved(accel.clone()));
        }
        if let Some(other) = seen.insert(n, id) {
            return Err(KeysError::Clash(accel.clone(), other.into(), id.clone()));
        }
    }
    Ok(kept)
}

/// `actions` with the person's chords in place.
pub fn apply<'a>(
    defaults: &'a [(String, String)],
    bindings: &'a Bindings,
) -> Vec<(&'a str, &'a str)> {
    defaults
        .iter()
        .map(|(id, d)| {
            (
                id.as_str(),
                bindings.get(id).map_or(d.as_str(), String::as_str),
            )
        })
        .collect()
}
