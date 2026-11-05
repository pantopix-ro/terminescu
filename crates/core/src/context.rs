//! A side tab's pins and extracted variables (task 4.4, D12, D13). Typed variables with storage
//! modes come with task 6.1; this module covers what the transcript pane creates.

use crate::db::{Db, Result};
use crate::state::Id;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Longest text a pin or a variable keeps, in characters.
pub const MAX_TEXT: usize = 20_000;
/// Context around a pinned quote that re-anchors it (D12).
pub const QUOTE_CONTEXT: usize = 40;

/// A pin on a transcript entry, whole or a selection in it (D12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    #[serde(default)]
    pub id: Id,
    pub side_tab_id: Id,
    pub session: String,
    pub entry_id: String,
    /// The pinned text; for a whole entry, its text.
    pub exact: String,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    /// UTF-16 offsets of `exact` in the entry text, as the webview counts them; absent for a
    /// whole entry. Stored only: the core never slices with them.
    #[serde(default)]
    pub start: Option<i64>,
    #[serde(default)]
    pub end: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub id: Id,
    pub side_tab_id: Id,
    pub name: String,
    pub value: String,
    pub source: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("{0} is not a variable name: letters, digits and _, not starting with a digit")]
    BadName(String),
    #[error("side tab {0} already has a variable {1}")]
    NameTaken(Id, String),
    #[error("{0} is not a Claude session id")]
    BadSession(String),
    #[error("nothing to save")]
    Empty,
    #[error(transparent)]
    Db(#[from] crate::db::DbError),
}

impl From<rusqlite::Error> for ContextError {
    fn from(e: rusqlite::Error) -> Self {
        ContextError::Db(e.into())
    }
}

fn cut(s: &str, chars: usize) -> String {
    s.chars().take(chars).collect()
}

/// `branch`, `pr_api`, `API_TOKEN`: what `$NAME` and `{{name}}` accept. `TERMINESCU_*` is ours.
pub fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 64
        && !name.starts_with("TERMINESCU_")
}

pub fn add_pin(db: &Db, pin: &Pin) -> std::result::Result<Pin, ContextError> {
    if !crate::runtime::is_session_id(&pin.session) {
        return Err(ContextError::BadSession(pin.session.clone()));
    }
    if pin.exact.trim().is_empty() {
        return Err(ContextError::Empty);
    }
    let owner = db.local_owner()?;
    let mut pin = pin.clone();
    pin.id = crate::new_id();
    pin.created_at = crate::now_ms();
    pin.exact = cut(&pin.exact, MAX_TEXT);
    pin.prefix = pin.prefix.map(|p| cut(&p, QUOTE_CONTEXT));
    pin.suffix = pin.suffix.map(|p| cut(&p, QUOTE_CONTEXT));
    db.conn().execute(
        "INSERT INTO pins (id, owner_id, side_tab_id, session_id, entry_uuid, quote_exact,
           quote_prefix, quote_suffix, start_offset, end_offset, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            pin.id,
            owner,
            pin.side_tab_id,
            pin.session,
            pin.entry_id,
            pin.exact,
            pin.prefix,
            pin.suffix,
            pin.start,
            pin.end,
            pin.created_at
        ],
    )?;
    Ok(pin)
}

pub fn pins(db: &Db, side_tab: &str) -> Result<Vec<Pin>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, side_tab_id, session_id, entry_uuid, quote_exact, quote_prefix, quote_suffix,
           start_offset, end_offset, created_at
         FROM pins WHERE owner_id = ?1 AND side_tab_id = ?2 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map(params![owner, side_tab], |r| {
        Ok(Pin {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            session: r.get(2)?,
            entry_id: r.get(3)?,
            exact: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            prefix: r.get(5)?,
            suffix: r.get(6)?,
            start: r.get(7)?,
            end: r.get(8)?,
            created_at: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The side tab of a removed pin, or `None` when there was no such pin.
pub fn remove_pin(db: &Db, id: &str) -> Result<Option<Id>> {
    let owner = db.local_owner()?;
    let side: Option<Id> = db
        .conn()
        .query_row(
            "DELETE FROM pins WHERE owner_id = ?1 AND id = ?2 RETURNING side_tab_id",
            params![owner, id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(side)
}

/// Saves extracted text as a variable of the side tab (D13). A name already in use is refused,
/// so an extract never overwrites a value.
pub fn add_variable(
    db: &Db,
    side_tab: &str,
    name: &str,
    value: &str,
    source: &str,
) -> std::result::Result<Variable, ContextError> {
    if !is_variable_name(name) {
        return Err(ContextError::BadName(name.into()));
    }
    if value.trim().is_empty() {
        return Err(ContextError::Empty);
    }
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let id = crate::new_id();
    let conn = db.conn();
    let taken: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM context_items
           WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'variable' AND name = ?3)",
        params![owner, side_tab, name],
        |r| r.get(0),
    )?;
    if taken {
        return Err(ContextError::NameTaken(side_tab.into(), name.into()));
    }
    let value = cut(value, MAX_TEXT);
    conn.execute(
        "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source, position,
           created_at, updated_at)
         VALUES (?1, ?2, ?3, 'variable', ?4, ?5, ?6,
           (SELECT COALESCE(MAX(position) + 1, 0) FROM context_items WHERE side_tab_id = ?3), ?7, ?7)",
        params![id, owner, side_tab, name, value, source, now],
    )?;
    Ok(Variable {
        id,
        side_tab_id: side_tab.into(),
        name: name.into(),
        value,
        source: Some(source.into()),
    })
}

pub fn variables(db: &Db, side_tab: &str) -> Result<Vec<Variable>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, side_tab_id, name, value, source FROM context_items
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'variable' ORDER BY position",
    )?;
    let rows = stmt.query_map(params![owner, side_tab], |r| {
        Ok(Variable {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            name: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            value: r.get(3)?,
            source: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::{Saved, save};
    use crate::state::{SideTab, Tab, Window, Workspace};

    fn db_with_side_tab() -> Db {
        let db = Db::open_in_memory().unwrap();
        let workspace = Workspace {
            windows: vec![Window {
                id: "w1".into(),
                side_tabs: vec![SideTab {
                    status: None,
                    id: "s1".into(),
                    title: Some("PROJ-482".into()),
                    tabs: vec![Tab {
                        status: None,
                        id: "t1".into(),
                        title: None,
                        layout: crate::layout::Layout::pane("p1"),
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

    fn pin(exact: &str) -> Pin {
        Pin {
            id: String::new(),
            side_tab_id: "s1".into(),
            session: "sess-0b5c".into(),
            entry_id: "00000000-0000-4000-9000-000000000006".into(),
            exact: exact.into(),
            prefix: Some("x".repeat(100)),
            suffix: None,
            start: Some(12),
            end: Some(12 + exact.len() as i64),
            created_at: 0,
        }
    }

    #[test]
    fn a_pin_comes_back_with_its_quote_and_goes_away() {
        let db = db_with_side_tab();
        let saved = add_pin(&db, &pin("batches of 5,000 rows on a cursor over id")).unwrap();
        assert_eq!(saved.prefix.as_deref().map(str::len), Some(QUOTE_CONTEXT));
        assert_eq!(pins(&db, "s1").unwrap(), vec![saved.clone()]);
        assert_eq!(remove_pin(&db, &saved.id).unwrap().as_deref(), Some("s1"));
        assert!(pins(&db, "s1").unwrap().is_empty());
        let mut bad = pin("text");
        bad.session = "--resume".into();
        assert!(matches!(
            add_pin(&db, &bad),
            Err(ContextError::BadSession(_))
        ));
    }

    #[test]
    fn an_extract_never_overwrites_a_variable() {
        let db = db_with_side_tab();
        let url = "https://github.com/acme/api/pull/918";
        let v = add_variable(&db, "s1", "pr_api", url, "extracted").unwrap();
        assert_eq!(v.name, "pr_api");
        assert!(matches!(
            add_variable(&db, "s1", "pr_api", "other", "extracted"),
            Err(ContextError::NameTaken(..))
        ));
        assert!(matches!(
            add_variable(&db, "s1", "9lives", "x", "extracted"),
            Err(ContextError::BadName(_))
        ));
        assert!(matches!(
            add_variable(&db, "s1", "TERMINESCU_PANE_ID", "x", "extracted"),
            Err(ContextError::BadName(_))
        ));
        assert_eq!(variables(&db, "s1").unwrap(), vec![v]);
    }
}
