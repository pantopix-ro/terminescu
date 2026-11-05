//! The Files section (D30): files Claude wrote or edited in a side tab, from `PostToolUse` hook
//! events for Write, Edit, MultiEdit and NotebookEdit. One entry per path; a file Claude
//! created stays "added" through later edits.

use crate::db::{Db, Result};
use crate::state::Id;
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Added,
    Modified,
}

impl Change {
    fn as_str(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Modified => "modified",
        }
    }
}

/// What one hook event touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touch {
    /// Absolute.
    pub path: String,
    pub change: Change,
    pub session: Option<String>,
    /// The session's cwd, which the path shows relative to.
    pub cwd: Option<String>,
}

/// The file a `PostToolUse` event for a file-editing tool touched, if any.
pub fn touched(event: &serde_json::Value) -> Option<Touch> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    if s(event, "hook_event_name")? != "PostToolUse" {
        return None;
    }
    let tool = s(event, "tool_name")?;
    let input = event.get("tool_input")?;
    let path = match tool.as_str() {
        "Write" | "Edit" | "MultiEdit" => s(input, "file_path")?,
        "NotebookEdit" => s(input, "notebook_path")?,
        _ => return None,
    };
    let cwd = s(event, "cwd").filter(|c| Path::new(c).is_absolute());
    let path = if Path::new(&path).is_absolute() {
        path
    } else {
        Path::new(cwd.as_deref()?).join(&path).display().to_string()
    };
    // Write reports `"type": "create"` for a new file and `"update"` over an old one.
    let created = tool == "Write"
        && event
            .get("tool_response")
            .and_then(|r| r.get("type"))
            .and_then(|t| t.as_str())
            == Some("create");
    Some(Touch {
        path,
        change: if created {
            Change::Added
        } else {
            Change::Modified
        },
        session: s(event, "session_id"),
        cwd,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchedFile {
    pub id: Id,
    pub side_tab_id: Id,
    /// Absolute.
    pub path: String,
    /// `path` relative to the session's cwd when it lies under it, else `path`.
    pub shown: String,
    pub change: Change,
    pub origin_tab_id: Option<Id>,
    pub updated_at: i64,
}

/// Records a touch for a side tab. A path seen before keeps its entry and its "added".
pub fn record(db: &Db, side_tab: &str, origin_tab: Option<&str>, t: &Touch) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "INSERT INTO context_files
           (id, owner_id, side_tab_id, path, change, session_id, updated_at, origin_tab_id, cwd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (side_tab_id, path) DO UPDATE SET
           change = CASE WHEN change = 'added' THEN 'added' ELSE excluded.change END,
           session_id = excluded.session_id,
           updated_at = excluded.updated_at,
           origin_tab_id = excluded.origin_tab_id,
           cwd = excluded.cwd",
        params![
            crate::new_id(),
            owner,
            side_tab,
            t.path,
            t.change.as_str(),
            t.session,
            crate::now_ms(),
            origin_tab,
            t.cwd,
        ],
    )?;
    Ok(())
}

/// A side tab's touched files, last touched first.
pub fn list(db: &Db, side_tab: &str) -> Result<Vec<TouchedFile>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut st = conn.prepare(
        "SELECT id, side_tab_id, path, change, origin_tab_id, updated_at, cwd FROM context_files
          WHERE owner_id = ?1 AND side_tab_id = ?2 ORDER BY updated_at DESC, path",
    )?;
    let rows = st.query_map(params![owner, side_tab], |r| {
        let path: String = r.get(2)?;
        let cwd: Option<String> = r.get(6)?;
        Ok(TouchedFile {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            shown: shorten(&path, cwd.as_deref()),
            path,
            change: if r.get::<_, String>(3)? == "added" {
                Change::Added
            } else {
                Change::Modified
            },
            origin_tab_id: r.get(4)?,
            updated_at: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Drops one entry; the file stays on disk.
pub fn remove(db: &Db, id: &str) -> Result<Option<Id>> {
    use rusqlite::OptionalExtension;
    let owner = db.local_owner()?;
    Ok(db
        .conn()
        .query_row(
            "DELETE FROM context_files WHERE owner_id = ?1 AND id = ?2 RETURNING side_tab_id",
            params![owner, id],
            |r| r.get(0),
        )
        .optional()?)
}

fn shorten(path: &str, cwd: Option<&str>) -> String {
    cwd.and_then(|c| Path::new(path).strip_prefix(c).ok())
        .map(|p| p.display().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| path.to_string())
}
