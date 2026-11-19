//! A side tab's context: saved variables, links, text and pins (D12, D13, D21). Session and
//! secret variables live in the daemon; [`crate::vars`] joins both stores.

use crate::db::{Db, Result};
use crate::links::{self, LinkType, NameRank};
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

/// A link or a piece of text kept in the context. A link carries its best name so far, and
/// the label the panel shows (refine-context-links D5, D6).
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
    pub name: Option<String>,
    /// The name, else [`links::label`] for a link, else the text.
    pub label: String,
    pub link_type: Option<LinkType>,
    /// A plugin's mark on a link, as a PR's state. Memory only; the app fills it in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<terminescu_plugin_api::LinkBadge>,
}

impl Item {
    /// One row's columns, with the label and type worked out.
    #[allow(clippy::too_many_arguments)]
    fn from_row(
        id: Id,
        side_tab_id: Id,
        kind: ItemKind,
        value: String,
        source: Option<String>,
        origin_tab_id: Option<Id>,
        created_at: i64,
        name: Option<String>,
    ) -> Self {
        let (label, link_type) = match kind {
            ItemKind::Link => (
                name.clone().unwrap_or_else(|| links::label(&value)),
                Some(links::link_type(&value)),
            ),
            ItemKind::Text => (value.clone(), None),
        };
        Item {
            id,
            side_tab_id,
            kind,
            value,
            source,
            origin_tab_id,
            created_at,
            name,
            label,
            link_type,
            badge: None,
        }
    }
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

/// `branch`, `pr_api`, `API_TOKEN`: what `$NAME` and `{{name}}` accept. `TERMINESCU_*` is ours,
/// and names the shell acts on, as `PATH`, stay the shell's.
pub fn is_variable_name(name: &str) -> bool {
    name.len() <= 64 && terminescu_protocol::is_var_name(name)
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

/// Adds a link or a piece of text. The same link or text twice in one side tab is kept once.
pub fn add_item(
    db: &Db,
    side_tab: &str,
    kind: ItemKind,
    value: &str,
    source: &str,
    origin_tab: Option<&str>,
) -> std::result::Result<Item, ContextError> {
    add_named_item(db, side_tab, kind, value, source, origin_tab, None).map(|(item, _)| item)
}

/// [`add_item`] with a name for a link and its source. A link is cleaned first
/// ([`links::clean`]); one the side tab already holds keeps its row and only upgrades its name
/// (D3, D5). The flag says whether a new row landed.
pub fn add_named_item(
    db: &Db,
    side_tab: &str,
    kind: ItemKind,
    value: &str,
    source: &str,
    origin_tab: Option<&str>,
    name: Option<(&str, NameRank)>,
) -> std::result::Result<(Item, bool), ContextError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ContextError::Empty);
    }
    let value = match kind {
        ItemKind::Link => {
            links::clean(value).ok_or_else(|| ContextError::BadLink(cut(value, 80)))?
        }
        ItemKind::Text => cut(value, MAX_TEXT),
    };
    let name = match (kind, name) {
        (ItemKind::Link, Some((n, rank))) if rank > NameRank::None => {
            links::tidy_name(n).map(|n| (n, rank))
        }
        _ => None,
    };
    if let Some(found) = items(db, side_tab)?
        .into_iter()
        .find(|i| i.kind == kind && i.value == value)
    {
        if let Some((n, rank)) = &name
            && name_link(db, side_tab, &value, n, *rank)?
        {
            let again = items(db, side_tab)?.into_iter().find(|i| i.id == found.id);
            return Ok((again.unwrap_or(found), false));
        }
        return Ok((found, false));
    }
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let id = crate::new_id();
    let (stored_name, rank) = match &name {
        Some((n, r)) => (Some(n.clone()), *r as i64),
        None => (None, 0),
    };
    db.conn().execute(
        "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source,
           origin_tab_id, position, created_at, updated_at, name_rank)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
           (SELECT COALESCE(MAX(position) + 1, 0) FROM context_items WHERE side_tab_id = ?3),
           ?9, ?9, ?10)",
        params![
            id,
            owner,
            side_tab,
            kind.as_str(),
            stored_name,
            value,
            source,
            origin_tab,
            now,
            rank
        ],
    )?;
    let item = Item::from_row(
        id,
        side_tab.into(),
        kind,
        value,
        Some(source.into()),
        origin_tab.map(Into::into),
        now,
        stored_name,
    );
    Ok((item, true))
}

/// Gives every copy of a link in a side tab `name`, unless a copy has a name from a stronger
/// source (D5). Returns whether a row changed.
pub fn name_link(db: &Db, side_tab: &str, url: &str, name: &str, rank: NameRank) -> Result<bool> {
    let Some(name) = links::tidy_name(name) else {
        return Ok(false);
    };
    let owner = db.local_owner()?;
    let stronger: bool = db.conn().query_row(
        "SELECT EXISTS (SELECT 1 FROM context_items WHERE owner_id = ?1 AND side_tab_id = ?2
           AND kind = 'link' AND value = ?3 AND name_rank > ?4)",
        params![owner, side_tab, url, rank as i64],
        |r| r.get(0),
    )?;
    if stronger {
        return Ok(false);
    }
    let changed = db.conn().execute(
        "UPDATE context_items SET name = ?4, name_rank = ?5, updated_at = ?6
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind = 'link' AND value = ?3
           AND (name IS NOT ?4 OR name_rank <> ?5)",
        params![owner, side_tab, url, name, rank as i64, crate::now_ms()],
    )?;
    Ok(changed > 0)
}

/// The side tab's link URLs, once each, in panel order.
pub fn link_urls(db: &Db, side_tab: &str) -> Result<Vec<String>> {
    Ok(items(db, side_tab)?
        .into_iter()
        .filter(|i| i.kind == ItemKind::Link)
        .map(|i| i.value)
        .collect())
}

pub fn items(db: &Db, side_tab: &str) -> Result<Vec<Item>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, side_tab_id, kind, value, source, origin_tab_id, created_at, name, name_rank
         FROM context_items
         WHERE owner_id = ?1 AND side_tab_id = ?2 AND kind IN ('link', 'text') ORDER BY position",
    )?;
    let rows = stmt.query_map(params![owner, side_tab], |r| {
        let kind = if r.get::<_, String>(2)? == "link" {
            ItemKind::Link
        } else {
            ItemKind::Text
        };
        let item = Item::from_row(
            r.get(0)?,
            r.get(1)?,
            kind,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
        );
        Ok((item, r.get::<_, i64>(8)?))
    })?;
    // A merge or a moved tab can bring a link the side tab already has: it shows once, in the
    // first copy's place, with the best name among the copies.
    let mut all: Vec<(Item, i64)> = Vec::new();
    for row in rows {
        let (item, rank) = row?;
        match all
            .iter_mut()
            .find(|(i, _)| i.kind == item.kind && i.value == item.value)
        {
            Some((first, best)) => {
                if rank > *best && item.name.is_some() {
                    *best = rank;
                    first.label = item.label.clone();
                    first.name = item.name;
                }
            }
            None => all.push((item, rank)),
        }
    }
    Ok(all.into_iter().map(|(i, _)| i).collect())
}

/// Cleans every saved link once per database (D4): each value takes its cleaned form, and the
/// copies of one link in a side tab share the best name. Nothing is deleted; copies move with
/// their tabs. Returns how many rows changed.
pub fn clean_saved_links(db: &Db) -> Result<usize> {
    const DONE: &str = "links.cleaned.1";
    let owner = db.local_owner()?;
    let conn = db.conn();
    let done: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM app_state WHERE owner_id = ?1 AND key = ?2)",
        params![owner, DONE],
        |r| r.get(0),
    )?;
    if done {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    let rows: Vec<(Id, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, value FROM context_items WHERE owner_id = ?1 AND kind = 'link'")?;
        stmt.query_map(params![owner], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut changed = 0;
    for (id, value) in rows {
        if let Some(clean) = links::clean(&value).filter(|c| *c != value) {
            changed += tx.execute(
                "UPDATE context_items SET value = ?2 WHERE id = ?1",
                params![id, clean],
            )?;
        }
    }
    // Every copy takes the name of the strongest named copy of its side tab and value.
    changed += tx.execute(
        "UPDATE context_items AS c SET (name, name_rank) = (
           SELECT b.name, b.name_rank FROM context_items b
           WHERE b.owner_id = c.owner_id AND b.side_tab_id = c.side_tab_id AND b.kind = 'link'
             AND b.value = c.value AND b.name IS NOT NULL
           ORDER BY b.name_rank DESC, b.updated_at DESC LIMIT 1)
         WHERE c.owner_id = ?1 AND c.kind = 'link' AND EXISTS (
           SELECT 1 FROM context_items b
           WHERE b.owner_id = c.owner_id AND b.side_tab_id = c.side_tab_id AND b.kind = 'link'
             AND b.value = c.value AND b.name IS NOT NULL AND b.name_rank > c.name_rank)",
        params![owner],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO app_state (owner_id, key, value) VALUES (?1, ?2, '1')",
        params![owner, DONE],
    )?;
    tx.commit()?;
    Ok(changed)
}

/// The side tab of a removed link or text. Copies of the same value in that side tab go too,
/// so a hidden copy never shows up in its place.
pub fn remove_item(db: &Db, id: &str) -> Result<Option<Id>> {
    let owner = db.local_owner()?;
    Ok(db
        .conn()
        .query_row(
            "DELETE FROM context_items
             WHERE owner_id = ?1 AND kind IN ('link', 'text')
               AND (side_tab_id, kind, value) =
                   (SELECT side_tab_id, kind, value FROM context_items
                    WHERE owner_id = ?1 AND id = ?2 AND kind IN ('link', 'text'))
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
                zoom_percent: None,
                side_tabs: vec![SideTab {
                    folder: false,
                    accent: None,
                    name: None,
                    status: None,
                    icon: None,
                    id: "s1".into(),
                    title: Some("PROJ-482".into()),
                    tabs: vec![Tab {
                        accent: None,
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
    fn a_link_or_text_shows_once_and_goes_with_its_copies() {
        let db = db_with_side_tab();
        let t = add_item(&db, "s1", ItemKind::Text, "api, web", "typed", None).unwrap();
        let again = add_item(&db, "s1", ItemKind::Text, " api, web ", "auto", None).unwrap();
        assert_eq!(t.id, again.id);
        let url = "https://example.test/acme/api/pull/482";
        let a = add_item(&db, "s1", ItemKind::Link, url, "pasted", None).unwrap();
        // A merge moves rows without the check: a copy can land beside the first.
        let owner = db.local_owner().unwrap();
        db.conn()
            .execute(
                "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, source,
                   position, created_at, updated_at)
                 VALUES ('copy', ?1, 's1', 'link', NULL, ?2, 'auto', 99, 0, 0)",
                params![owner, url],
            )
            .unwrap();
        assert_eq!(items(&db, "s1").unwrap(), vec![t, a.clone()]);
        assert_eq!(remove_item(&db, &a.id).unwrap().as_deref(), Some("s1"));
        assert_eq!(items(&db, "s1").unwrap().len(), 1);
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
