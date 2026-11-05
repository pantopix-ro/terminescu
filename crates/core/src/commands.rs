//! Saved commands (D24): shell lines with `{{variables}}`, kept for one side tab or globally.
//! "Run" types the line and Enter, "Insert" types it without Enter, into the focused pane, a new
//! tab or a new side tab. A secret goes in as `$NAME` and the shell expands it (D35).

use crate::context::{Mode, Variable};
use crate::db::{Db, DbError};
use crate::state::Id;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest command body, in characters.
pub const MAX_BODY: usize = 10_000;
/// Longest command name, in characters.
pub const MAX_NAME: usize = 80;

const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

/// Where a command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    #[default]
    Focused,
    NewTab,
    NewSideTab,
}

impl Target {
    fn as_str(self) -> &'static str {
        match self {
            Target::Focused => "focused",
            Target::NewTab => "new_tab",
            Target::NewSideTab => "new_side_tab",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "new_tab" => Target::NewTab,
            "new_side_tab" => Target::NewSideTab,
            _ => Target::Focused,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedCommand {
    pub id: Id,
    /// `None` for a global command, in the library.
    pub side_tab_id: Option<Id>,
    pub name: String,
    pub body: String,
    pub target: Target,
    /// A launcher command runs in its own tab when saved and after a reboot, and the side tab
    /// shows how many of them are up.
    pub launcher: bool,
    /// The tab a launcher command runs in, while it exists.
    pub tab_id: Option<Id>,
    pub position: i64,
    pub created_at: i64,
}

/// A command to save, or new values for one.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCommand {
    #[serde(default)]
    pub side_tab_id: Option<Id>,
    pub name: String,
    pub body: String,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub launcher: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandStoreError {
    #[error("a command needs a name of at most {MAX_NAME} characters")]
    BadName,
    #[error("the command is empty")]
    Empty,
    #[error("the command is longer than {MAX_BODY} characters")]
    TooLong,
    #[error("no command {0}")]
    NoCommand(String),
    #[error("a launcher command belongs to one side tab")]
    GlobalLauncher,
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<rusqlite::Error> for CommandStoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.into())
    }
}

pub type Result<T> = std::result::Result<T, CommandStoreError>;

fn check(new: &NewCommand) -> Result<(String, String)> {
    if new.launcher && new.side_tab_id.is_none() {
        return Err(CommandStoreError::GlobalLauncher);
    }
    let name = new.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(CommandStoreError::BadName);
    }
    // A trailing line break would run the line on Insert; the Run button adds the one Enter.
    let body = new.body.trim_end_matches(['\r', '\n']).to_string();
    if body.trim().is_empty() {
        return Err(CommandStoreError::Empty);
    }
    if body.chars().count() > MAX_BODY {
        return Err(CommandStoreError::TooLong);
    }
    Ok((name.to_string(), body))
}

const SELECT: &str =
    "SELECT id, side_tab_id, name, body, target, launcher, position, created_at, tab_id
   FROM commands";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SavedCommand> {
    let target: String = r.get(4)?;
    Ok(SavedCommand {
        id: r.get(0)?,
        side_tab_id: r.get(1)?,
        name: r.get(2)?,
        body: r.get(3)?,
        target: Target::parse(&target),
        launcher: r.get::<_, i64>(5)? == 1,
        position: r.get(6)?,
        created_at: r.get(7)?,
        tab_id: r.get(8)?,
    })
}

/// A side tab's own commands first, then the global ones, each in saved order.
pub fn list(db: &Db, side_tab: Option<&str>) -> Result<Vec<SavedCommand>> {
    let owner = db.local_owner()?;
    let mut stmt = db.conn().prepare(&format!(
        "{SELECT} WHERE owner_id = ?1 AND (side_tab_id IS NULL OR side_tab_id = ?2)
         ORDER BY side_tab_id IS NULL, position, created_at"
    ))?;
    let rows = stmt.query_map(params![owner, side_tab], row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Every command, global ones first, then each side tab's in order (Settings › Commands).
pub fn all(db: &Db) -> Result<Vec<SavedCommand>> {
    let owner = db.local_owner()?;
    let mut stmt = db.conn().prepare(&format!(
        "{SELECT} WHERE owner_id = ?1
         ORDER BY side_tab_id IS NOT NULL, side_tab_id, position, created_at"
    ))?;
    let rows = stmt.query_map(params![owner], row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(db: &Db, id: &str) -> Result<SavedCommand> {
    let owner = db.local_owner()?;
    db.conn()
        .query_row(
            &format!("{SELECT} WHERE owner_id = ?1 AND id = ?2"),
            params![owner, id],
            row,
        )
        .optional()?
        .ok_or_else(|| CommandStoreError::NoCommand(id.into()))
}

pub fn create(db: &Db, new: &NewCommand) -> Result<SavedCommand> {
    let (name, body) = check(new)?;
    let owner = db.local_owner()?;
    let id = crate::new_id();
    let position: i64 = db.conn().query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM commands
          WHERE owner_id = ?1 AND side_tab_id IS ?2",
        params![owner, new.side_tab_id],
        |r| r.get(0),
    )?;
    db.conn().execute(
        "INSERT INTO commands (id, owner_id, side_tab_id, name, body, target, launcher, position,
                               created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            owner,
            new.side_tab_id,
            name,
            body,
            new.target.as_str(),
            new.launcher as i64,
            position,
            crate::now_ms()
        ],
    )?;
    get(db, &id)
}

/// New name, body, target and launcher flag; the scope and the place stay.
/// Changes name, body, target and the launcher flag. The side tab stays the stored one, so the
/// launcher check runs against it, not against `new.side_tab_id`.
pub fn update(db: &Db, id: &str, new: &NewCommand) -> Result<SavedCommand> {
    let stored = get(db, id)?;
    let (name, body) = check(&NewCommand {
        side_tab_id: stored.side_tab_id.clone(),
        ..new.clone()
    })?;
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "UPDATE commands SET name = ?3, body = ?4, target = ?5, launcher = ?6
          WHERE owner_id = ?1 AND id = ?2",
        params![
            owner,
            id,
            name,
            body,
            new.target.as_str(),
            new.launcher as i64
        ],
    )?;
    if n == 0 {
        return Err(CommandStoreError::NoCommand(id.into()));
    }
    get(db, id)
}

/// Every launcher command, in each side tab's saved order.
pub fn launchers(db: &Db) -> Result<Vec<SavedCommand>> {
    let owner = db.local_owner()?;
    let mut stmt = db.conn().prepare(&format!(
        "{SELECT} WHERE owner_id = ?1 AND launcher = 1 ORDER BY side_tab_id, position, created_at"
    ))?;
    let rows = stmt.query_map(params![owner], row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Remembers the tab a launcher command runs in.
pub fn set_tab(db: &Db, id: &str, tab: Option<&str>) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "UPDATE commands SET tab_id = ?3 WHERE owner_id = ?1 AND id = ?2",
        params![owner, id, tab],
    )?;
    Ok(())
}

pub fn delete(db: &Db, id: &str) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "DELETE FROM commands WHERE owner_id = ?1 AND id = ?2",
        params![owner, id],
    )?;
    Ok(())
}

/// A command with its variables filled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub text: String,
    /// Names with no value in the side tab or `extra`, in order of first use.
    pub missing: Vec<String>,
    /// Names whose value holds a line break or a control character: Run refuses those, since
    /// Enter would run more than the person read.
    pub unsafe_for_run: Vec<String>,
}

/// Fills `{{name}}` from the side tab's variables, then from `extra` (values typed when asked).
/// A secret becomes `$NAME` (D35). A missing name stays as `{{name}}`.
pub fn resolve(body: &str, vars: &[Variable], extra: &BTreeMap<String, String>) -> Resolved {
    let mut out = String::with_capacity(body.len());
    let mut missing: Vec<String> = Vec::new();
    let mut unsafe_for_run: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let raw = &after[..end];
        let whole = &rest[start..start + 2 + end + 2];
        rest = &after[end + 2..];
        if raw.contains(['{', '}']) {
            out.push_str(whole);
            continue;
        }
        let name = raw.trim();
        let value = match vars.iter().find(|v| v.name == name) {
            Some(v) if v.mode == Mode::Secret => Some(format!("${}", v.name)),
            Some(v) => Some(v.value.clone().unwrap_or_default()),
            None => extra.get(name).filter(|s| !s.is_empty()).cloned(),
        };
        match value {
            Some(v) => {
                if v.chars().any(|c| c.is_control()) && !unsafe_for_run.iter().any(|n| n == name) {
                    unsafe_for_run.push(name.to_string());
                }
                out.push_str(&v);
            }
            None => {
                if crate::context::is_variable_name(name) && !missing.iter().any(|n| n == name) {
                    missing.push(name.to_string());
                }
                out.push_str(whole);
            }
        }
    }
    out.push_str(rest);
    Resolved {
        text: out,
        missing,
        unsafe_for_run,
    }
}

/// `text` with `\r\n` and `\r` as `\n`, and every control character but `\n` and `\t` gone:
/// C0, DEL and C1 (which some terminals read as CSI).
pub fn typeable(text: &str) -> String {
    // Whole markers go first, until none is left: removing one can join the halves of another.
    let mut text = text.to_string();
    loop {
        let next = text.replace(PASTE_START, "").replace(PASTE_END, "");
        if next == text {
            break;
        }
        text = next;
    }
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect()
}

/// The bytes a command types into a shell, as `pasteBytes` in packages/ui does without a known
/// paste mode: line breaks become `\n` and never `\r`, more than one line goes in as a bracketed
/// paste, and every other control character is dropped, ESC included, so no escape sequence
/// (a paste end marker above all) can end the paste early. `run` adds the one `\r`.
pub fn type_bytes(text: &str, run: bool) -> Vec<u8> {
    let clean = typeable(text);
    let mut out = if clean.contains('\n') {
        format!("{PASTE_START}{clean}{PASTE_END}")
    } else {
        clean
    };
    if run {
        out.push('\r');
    }
    out.into_bytes()
}
