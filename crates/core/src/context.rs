//! A side tab's context: saved variables, links, text and pins (D12, D13, D21). Session and
//! secret variables live in the daemon; [`crate::vars`] joins both stores.

use crate::db::{Db, Result};
use crate::state::Id;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Longest text a pin, an item or a variable keeps, in characters.
pub const MAX_TEXT: usize = 20_000;
/// Context around a pinned quote that re-anchors it (D12).
pub const QUOTE_CONTEXT: usize = 40;

/// Where a variable lives (D21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// In SQLite.
    Saved,
    /// In the daemon's memory only; gone with a reboot or its side tab.
    Session,
    /// Session, masked on screen, inserted as `$NAME`.
    Secret,
}

/// A pin on a transcript entry, whole or a selection in it, or on rows of a terminal pane (D12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    #[serde(default)]
    pub id: Id,
    pub side_tab_id: Id,
    /// The tab the pin came from; it moves with that tab (group 8).
    #[serde(default)]
    pub origin_tab_id: Option<Id>,
    /// Transcript pins: the session and the entry.
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    /// Terminal pins: the pane and its buffer rows, first and last.
    #[serde(default)]
    pub pane_id: Option<Id>,
    #[serde(default)]
    pub row_start: Option<i64>,
    #[serde(default)]
    pub row_end: Option<i64>,
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
    /// `None` for a secret: its value never leaves the daemon for the UI.
    pub value: Option<String>,
    pub mode: Mode,
    pub source: Option<String>,
    pub origin_tab_id: Option<Id>,
    /// When a TTL removes it, Unix ms.
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Link,
    Text,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            ItemKind::Link => "link",
            ItemKind::Text => "text",
        }
    }
}

/// A link or a piece of text kept in the context without a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: Id,
    pub side_tab_id: Id,
    pub kind: ItemKind,
    pub value: String,
    pub source: Option<String>,
    pub origin_tab_id: Option<Id>,
    pub created_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("{0} is not a variable name: letters, digits and _, not starting with a digit")]
    BadName(String),
    #[error("this side tab already has a variable {1}")]
    NameTaken(Id, String),
    #[error("no variable {0}")]
    NoVariable(String),
    #[error("{0} is not a Claude session id")]
    BadSession(String),
    #[error("a pin needs a session and entry, or a pane and rows")]
    BadPin,
    #[error("{0} is not an http(s) link")]
    BadLink(String),
    #[error("nothing to save")]
    Empty,
    #[error("{0} is a secret and stays one; remove it and add it again to change its mode")]
    SecretStays(String),
    #[error("type the value of {0} again to change its TTL")]
    NeedsValue(String),
    #[error("session and secret variables need the pty daemon: {0}")]
    NoDaemon(String),
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
    match (&pin.session, &pin.entry_id, &pin.pane_id) {
        (Some(s), Some(_), None) if !crate::runtime::is_session_id(s) => {
            return Err(ContextError::BadSession(s.clone()));
        }
        (Some(_), Some(_), None) => {}
        (None, None, Some(_)) if pin.row_start.is_some() && pin.row_end.is_some() => {}
        _ => return Err(ContextError::BadPin),
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
        "INSERT INTO pins (id, owner_id, side_tab_id, origin_tab_id, session_id, entry_uuid,
           pane_id, row_start, row_end, quote_exact, quote_prefix, quote_suffix, start_offset,
           end_offset, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            pin.id,
            owner,
            pin.side_tab_id,
            pin.origin_tab_id,
            pin.session,
            pin.entry_id,
            pin.pane_id,
            pin.row_start,
            pin.row_end,
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
        "SELECT id, side_tab_id, origin_tab_id, session_id, entry_uuid, pane_id, row_start,
           row_end, quote_exact, quote_prefix, quote_suffix, start_offset, end_offset, created_at
         FROM pins WHERE owner_id = ?1 AND side_tab_id = ?2 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map(params![owner, side_tab], |r| {
        Ok(Pin {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            origin_tab_id: r.get(2)?,
            session: r.get(3)?,
            entry_id: r.get(4)?,
            pane_id: r.get(5)?,
            row_start: r.get(6)?,
            row_end: r.get(7)?,
            exact: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            prefix: r.get(9)?,
            suffix: r.get(10)?,
            start: r.get(11)?,
            end: r.get(12)?,
            created_at: r.get(13)?,
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

/// Saves a variable in SQLite. A name in use is refused unless `replace`, so an extract never
/// overwrites a value; `terminescu var set` replaces.
pub fn put_saved(
    db: &Db,
    side_tab: &str,
    name: &str,
    value: &str,
    source: &str,
    origin_tab: Option<&str>,
    replace: bool,
) -> std::result::Result<Variable, ContextError> {
    if !is_variable_name(name) {
        return Err(ContextError::BadName(name.into()));
    }
    if value.trim().is_empty() {
        return Err(ContextError::Empty);
    }
    let value = cut(value, MAX_TEXT);
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let conn = db.conn();
    if let Some(old) = saved_variable(db, side_tab, name)? {
        if !replace {
            return Err(ContextError::NameTaken(side_tab.into(), name.into()));
        }
        conn.execute(
            "UPDATE context_items SET value = ?2, updated_at = ?3 WHERE id = ?1",
            params![old.id, value, now],
        )?;
        return Ok(Variable {
            value: Some(value),
            ..old
        });
    }
    let id = crate::new_id();
    conn.execute(
        "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source,
           origin_tab_id, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'variable', ?4, ?5, ?6, ?7,
           (SELECT COALESCE(MAX(position) + 1, 0) FROM context_items WHERE side_tab_id = ?3), ?8, ?8)",
        params![id, owner, side_tab, name, value, source, origin_tab, now],
    )?;
    Ok(Variable {
        id,
        side_tab_id: side_tab.into(),
        name: name.into(),
        value: Some(value),
        mode: Mode::Saved,
        source: Some(source.into()),
        origin_tab_id: origin_tab.map(Into::into),
        expires_at: None,
    })
}

fn variable_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Variable> {
    Ok(Variable {
        id: r.get(0)?,
        side_tab_id: r.get(1)?,
        name: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        value: Some(r.get(3)?),
        mode: Mode::Saved,
        source: r.get(4)?,
        origin_tab_id: r.get(5)?,
        expires_at: None,
    })
}

const VARIABLE_COLUMNS: &str = "id, side_tab_id, name, value, source, origin_tab_id";

pub fn saved_variable(db: &Db, side_tab: &str, name: &str) -> Result<Option<Variable>> {
    let owner = db.local_owner()?;
    Ok(db
        .conn()
        .query_row(
            &format!(
                "SELECT {VARIABLE_COLUMNS} FROM context_items
                 WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'variable' AND name = ?3"
            ),
            params![owner, side_tab, name],
            variable_row,
        )
        .optional()?)
}

/// The side tab's saved variables, in the order they were added.
pub fn saved_variables(db: &Db, side_tab: &str) -> Result<Vec<Variable>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(&format!(
        "SELECT {VARIABLE_COLUMNS} FROM context_items
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'variable' ORDER BY position"
    ))?;
    let rows = stmt.query_map(params![owner, side_tab], variable_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Whether a saved variable went.
pub fn remove_saved(db: &Db, side_tab: &str, name: &str) -> Result<bool> {
    let owner = db.local_owner()?;
    Ok(db.conn().execute(
        "DELETE FROM context_items
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'variable' AND name = ?3",
        params![owner, side_tab, name],
    )? > 0)
}

/// Adds a link or a piece of text. The same link twice in one side tab is kept once.
pub fn add_item(
    db: &Db,
    side_tab: &str,
    kind: ItemKind,
    value: &str,
    source: &str,
    origin_tab: Option<&str>,
) -> std::result::Result<Item, ContextError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ContextError::Empty);
    }
    let web = value.starts_with("https://") || value.starts_with("http://");
    if kind == ItemKind::Link && (!web || value.contains(char::is_whitespace)) {
        return Err(ContextError::BadLink(cut(value, 80)));
    }
    let value = cut(value, MAX_TEXT);
    if kind == ItemKind::Link
        && let Some(found) = items(db, side_tab)?
            .into_iter()
            .find(|i| i.kind == ItemKind::Link && i.value == value)
    {
        return Ok(found);
    }
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let id = crate::new_id();
    db.conn().execute(
        "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source,
           origin_tab_id, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7,
           (SELECT COALESCE(MAX(position) + 1, 0) FROM context_items WHERE side_tab_id = ?3), ?8, ?8)",
        params![id, owner, side_tab, kind.as_str(), value, source, origin_tab, now],
    )?;
    Ok(Item {
        id,
        side_tab_id: side_tab.into(),
        kind,
        value,
        source: Some(source.into()),
        origin_tab_id: origin_tab.map(Into::into),
        created_at: now,
    })
}

pub fn items(db: &Db, side_tab: &str) -> Result<Vec<Item>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, side_tab_id, kind, value, source, origin_tab_id, created_at FROM context_items
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind IN ('link', 'text') ORDER BY position",
    )?;
    let rows = stmt.query_map(params![owner, side_tab], |r| {
        Ok(Item {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            kind: if r.get::<_, String>(2)? == "link" {
                ItemKind::Link
            } else {
                ItemKind::Text
            },
            value: r.get(3)?,
            source: r.get(4)?,
            origin_tab_id: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The side tab of a removed link or text.
pub fn remove_item(db: &Db, id: &str) -> Result<Option<Id>> {
    let owner = db.local_owner()?;
    Ok(db
        .conn()
        .query_row(
            "DELETE FROM context_items WHERE owner_id = ?1 AND id = ?2 AND kind IN ('link', 'text')
             RETURNING side_tab_id",
            params![owner, id],
            |r| r.get(0),
        )
        .optional()?)
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
                    icon: None,
                    id: "s1".into(),
                    title: Some("PROJ-482".into()),
                    tabs: vec![Tab {
                        status: None,
                        icon: None,
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
            origin_tab_id: Some("t1".into()),
            session: Some("sess-0b5c".into()),
            entry_id: Some("00000000-0000-4000-9000-000000000006".into()),
            pane_id: None,
            row_start: None,
            row_end: None,
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
        bad.session = Some("--resume".into());
        assert!(matches!(
            add_pin(&db, &bad),
            Err(ContextError::BadSession(_))
        ));
        let mut half = pin("text");
        half.entry_id = None;
        assert!(matches!(add_pin(&db, &half), Err(ContextError::BadPin)));
    }

    #[test]
    fn a_terminal_pin_keeps_its_pane_and_rows() {
        let db = db_with_side_tab();
        let mut p = pin("curl -s :4800/audit/export | head");
        p.session = None;
        p.entry_id = None;
        p.pane_id = Some("p1".into());
        p.row_start = Some(120);
        p.row_end = Some(121);
        let saved = add_pin(&db, &p).unwrap();
        assert_eq!(pins(&db, "s1").unwrap(), vec![saved]);
        p.row_end = None;
        assert!(matches!(add_pin(&db, &p), Err(ContextError::BadPin)));
    }

    #[test]
    fn links_are_kept_once_and_text_is_kept_as_typed() {
        let db = db_with_side_tab();
        let url = "https://claude.ai/artifact/00000000-0000-4000-8000-000000000001";
        let a = add_item(&db, "s1", ItemKind::Link, url, "pasted", None).unwrap();
        let b = add_item(&db, "s1", ItemKind::Link, &format!(" {url} "), "auto", None).unwrap();
        assert_eq!(a.id, b.id);
        assert!(matches!(
            add_item(
                &db,
                "s1",
                ItemKind::Link,
                "ftp://example.com",
                "typed",
                None
            ),
            Err(ContextError::BadLink(_))
        ));
        let t = add_item(&db, "s1", ItemKind::Text, "api, web", "typed", Some("t1")).unwrap();
        assert_eq!(items(&db, "s1").unwrap(), vec![a, t.clone()]);
        assert_eq!(remove_item(&db, &t.id).unwrap().as_deref(), Some("s1"));
        assert_eq!(items(&db, "s1").unwrap().len(), 1);
    }

    #[test]
    fn an_extract_never_overwrites_a_variable() {
        let db = db_with_side_tab();
        let url = "https://github.com/acme/api/pull/918";
        let add = |name: &str, value: &str, replace| {
            put_saved(&db, "s1", name, value, "extracted", Some("t1"), replace)
        };
        let v = add("pr_api", url, false).unwrap();
        assert_eq!(v.name, "pr_api");
        assert!(matches!(
            add("pr_api", "other", false),
            Err(ContextError::NameTaken(..))
        ));
        assert!(matches!(
            add("9lives", "x", false),
            Err(ContextError::BadName(_))
        ));
        assert!(matches!(
            add("TERMINESCU_PANE_ID", "x", false),
            Err(ContextError::BadName(_))
        ));
        assert_eq!(saved_variables(&db, "s1").unwrap(), vec![v.clone()]);
        let replaced = add("pr_api", "https://github.com/acme/api/pull/919", true).unwrap();
        assert_eq!(replaced.id, v.id);
        assert!(remove_saved(&db, "s1", "pr_api").unwrap());
        assert!(!remove_saved(&db, "s1", "pr_api").unwrap());
    }
}
