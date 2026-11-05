//! Side tab merge (D5, D21): drop a side tab onto another and their contexts become one.
//! Tabs move in the workspace ([`crate::state::Command::MergeSideTab`]); this module moves
//! the context. Lists concatenate, variables union. A name both sides hold with different
//! values is a clash: keep the target's, or keep both with the source's renamed
//! (`branch_477`). A secret in both always clashes, since its value never comes back.
//!
//! [`merge`] returns an [`Undo`] that lives in memory for the undo window. It holds row ids,
//! old names and the saved rows it dropped, never a session or secret value: the source's
//! daemon variables that lose to the target wait in the daemon under [`parking`] instead.

use crate::context::{ContextError, Mode, Variable};
use crate::daemon::DaemonClient;
use crate::db::{Db, DbAccess};
use crate::state::Id;
use crate::vars;
use rusqlite::params;
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// A variable name both side tabs hold with different values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    pub name: String,
    /// The source's value; `None` for a secret.
    pub from_value: Option<String>,
    pub from_mode: Mode,
    pub into_value: Option<String>,
    pub into_mode: Mode,
    /// The name the source's variable takes when both stay.
    pub rename: String,
}

/// How the person settled the clashes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    /// Keep both, the source's renamed; otherwise the target's value wins.
    pub keep_both: bool,
    /// Names the person typed over the suggested renames, by clashing name.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
}

/// One row the merge deleted, to put back on undo.
#[derive(Debug, Clone)]
struct Row {
    table: &'static str,
    columns: Vec<String>,
    values: Vec<Value>,
}

/// What a merge did, to reverse it.
#[derive(Debug, Clone, Default)]
pub struct Undo {
    pub id: Id,
    pub from: Id,
    pub into: Id,
    /// Context items that moved, with their old position.
    items: Vec<(Id, i64)>,
    /// Saved variables renamed on the way, with their old name.
    renamed: Vec<(Id, String)>,
    /// Pins, touched files, commands, statuses and documents that moved, by table.
    moved: Vec<(&'static str, Id)>,
    dropped: Vec<Row>,
    /// Daemon variables moved into the target: old name, new name.
    daemon_moved: Vec<(String, String)>,
    /// Daemon variables parked for the undo window, by name.
    parked: Vec<String>,
    /// Renames done, old name to new, for the toast.
    pub renames: Vec<(String, String)>,
}

/// The daemon key that holds a merge's losing variables until its undo window ends.
pub fn parking(undo_id: &str) -> String {
    format!("merge-undo:{undo_id}")
}

/// The trailing number of a side tab's label: `PROJ-477` gives `477`.
fn ticket_number(label: Option<&str>) -> Option<&str> {
    let label = label?.trim_end();
    let digits = label
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| &label[i..])?;
    Some(digits)
}

/// `branch_477` from `branch` and `PROJ-477`, else `branch_2`; never a name in `taken`.
pub fn suggest(name: &str, label: Option<&str>, taken: &HashSet<String>) -> String {
    let base = match ticket_number(label) {
        Some(n) => format!("{name}_{n}"),
        None => format!("{name}_2"),
    };
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|c| !taken.contains(c))
        .expect("a free name")
}

fn same(a: &Variable, b: &Variable) -> bool {
    a.mode != Mode::Secret && b.mode != Mode::Secret && a.value == b.value
}

/// The clashes a merge of `from` into `into` would ask about. `from_label` is the source's
/// title as the sidebar shows it, for the suggested renames.
pub fn preview(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    from: &str,
    into: &str,
    from_label: Option<&str>,
) -> Result<Vec<Clash>, ContextError> {
    let a = vars::list(db, daemon, from)?;
    let b = vars::list(db, daemon, into)?;
    let mut taken: HashSet<String> = a.iter().chain(&b).map(|v| v.name.clone()).collect();
    let mut out = Vec::new();
    for v in &a {
        let Some(t) = b.iter().find(|t| t.name == v.name) else {
            continue;
        };
        if same(v, t) {
            continue;
        }
        let rename = suggest(&v.name, from_label, &taken);
        taken.insert(rename.clone());
        out.push(Clash {
            name: v.name.clone(),
            from_value: v.value.clone(),
            from_mode: v.mode,
            into_value: t.value.clone(),
            into_mode: t.mode,
            rename,
        });
    }
    Ok(out)
}

fn daemon_err(e: crate::daemon::DaemonError) -> ContextError {
    ContextError::NoDaemon(e.to_string())
}

/// Reads one row whole, then deletes it.
fn take(db: &Db, table: &'static str, id: &str) -> rusqlite::Result<Row> {
    let conn = db.conn();
    let mut st = conn.prepare(&format!("SELECT * FROM {table} WHERE id = ?1"))?;
    let columns: Vec<String> = st.column_names().map(String::from).collect();
    let n = columns.len();
    let values = st.query_row(params![id], |r| {
        (0..n)
            .map(|i| r.get::<_, Value>(i))
            .collect::<rusqlite::Result<Vec<_>>>()
    })?;
    conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![id])?;
    Ok(Row {
        table,
        columns,
        values,
    })
}

fn put_back(db: &Db, row: &Row) -> rusqlite::Result<()> {
    let marks = vec!["?"; row.values.len()].join(", ");
    db.conn().execute(
        &format!(
            "INSERT OR IGNORE INTO {} ({}) VALUES ({marks})",
            row.table,
            row.columns.join(", ")
        ),
        rusqlite::params_from_iter(row.values.iter()),
    )?;
    Ok(())
}

fn ids(db: &Db, sql: &str, p: impl rusqlite::Params) -> rusqlite::Result<Vec<Id>> {
    let mut st = db.conn().prepare(sql)?;
    let rows = st.query_map(p, |r| r.get(0))?;
    rows.collect()
}

/// Merges the context of `from` into `into`. Run it before the workspace command that closes
/// `from`, whose deletion would otherwise cascade to these rows.
pub fn merge(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    from: &str,
    into: &str,
    from_label: Option<&str>,
    resolution: &Resolution,
) -> Result<Undo, ContextError> {
    let clashes = preview(db, daemon, from, into, from_label)?;
    let source = vars::list(db, daemon, from)?;
    let target = vars::list(db, daemon, into)?;
    let mut undo = Undo {
        id: crate::new_id(),
        from: from.into(),
        into: into.into(),
        ..Default::default()
    };
    // The final name of each clashing source variable kept beside the target's.
    let mut rename: BTreeMap<String, String> = BTreeMap::new();
    if resolution.keep_both {
        let mut taken: HashSet<String> = source
            .iter()
            .chain(&target)
            .map(|v| v.name.clone())
            .collect();
        for c in &clashes {
            let to = resolution
                .renames
                .get(&c.name)
                .cloned()
                .unwrap_or_else(|| c.rename.clone());
            let daemon_held = c.from_mode != Mode::Saved;
            if !crate::context::is_variable_name(&to)
                || (daemon_held && !terminescu_protocol::is_var_name(&to))
            {
                return Err(ContextError::BadName(to));
            }
            if !taken.insert(to.clone()) {
                return Err(ContextError::NameTaken(into.into(), to));
            }
            rename.insert(c.name.clone(), to);
        }
    }
    let target_has = |name: &str| target.iter().any(|t| t.name == name);

    // The daemon first: a refusal there leaves both side tabs as they were.
    for v in source.iter().filter(|v| v.mode != Mode::Saved) {
        match (target_has(&v.name), rename.get(&v.name)) {
            (false, _) => undo.daemon_moved.push((v.name.clone(), v.name.clone())),
            (true, Some(to)) => undo.daemon_moved.push((v.name.clone(), to.clone())),
            (true, None) => undo.parked.push(v.name.clone()),
        }
    }
    if !undo.daemon_moved.is_empty() || !undo.parked.is_empty() {
        let d = daemon.ok_or_else(|| ContextError::NoDaemon("not connected".into()))?;
        let park: Vec<(String, String)> =
            undo.parked.iter().map(|n| (n.clone(), n.clone())).collect();
        if !park.is_empty() {
            d.side_vars_move(from, &parking(&undo.id), &park)
                .map_err(daemon_err)?;
        }
        if !undo.daemon_moved.is_empty()
            && let Err(e) = d.side_vars_move(from, into, &undo.daemon_moved)
        {
            let _ = d.side_vars_move(&parking(&undo.id), from, &park);
            return Err(daemon_err(e));
        }
    }

    let saved = db.with(|db| -> Result<(), ContextError> {
        let tx = db.conn().unchecked_transaction()?;
        let owner = db.local_owner()?;
        let offset: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM context_items WHERE side_tab_id = ?1",
            params![into],
            |r| r.get(0),
        )?;
        let links: HashSet<String> = crate::context::items(db, into)?
            .into_iter()
            .filter(|i| i.kind == crate::context::ItemKind::Link)
            .map(|i| i.value)
            .collect();
        let rows: Vec<(Id, String, Option<String>, String, i64)> = {
            let mut st = tx.prepare(
                "SELECT id, kind, name, value, position FROM context_items
                 WHERE owner_id = ?1 AND side_tab_id = ?2 ORDER BY position",
            )?;
            st.query_map(params![owner, from], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        for (id, kind, name, value, position) in rows {
            let name = name.unwrap_or_default();
            let drop = match kind.as_str() {
                "variable" if target_has(&name) => match rename.get(&name) {
                    Some(to) => {
                        tx.execute(
                            "UPDATE context_items SET name = ?2 WHERE id = ?1",
                            params![id, to],
                        )?;
                        undo.renamed.push((id.clone(), name.clone()));
                        false
                    }
                    None => true,
                },
                "link" => links.contains(&value),
                _ => false,
            };
            if drop {
                undo.dropped.push(take(db, "context_items", &id)?);
                continue;
            }
            tx.execute(
                "UPDATE context_items SET side_tab_id = ?2, position = ?3 WHERE id = ?1",
                params![id, into, position + offset],
            )?;
            undo.items.push((id, position));
        }
        // A file both side tabs touched keeps the target's entry.
        for id in ids(
            db,
            "SELECT f.id FROM context_files f WHERE f.owner_id = ?1 AND f.side_tab_id = ?2
               AND EXISTS (SELECT 1 FROM context_files g
                           WHERE g.side_tab_id = ?3 AND g.path = f.path)",
            params![owner, from, into],
        )? {
            undo.dropped.push(take(db, "context_files", &id)?);
        }
        for table in ["pins", "context_files", "commands", "statuses", "documents"] {
            for id in ids(
                db,
                &format!("SELECT id FROM {table} WHERE owner_id = ?1 AND side_tab_id = ?2"),
                params![owner, from],
            )? {
                tx.execute(
                    &format!("UPDATE {table} SET side_tab_id = ?2 WHERE id = ?1"),
                    params![id, into],
                )?;
                undo.moved.push((table, id));
            }
        }
        tx.commit()?;
        Ok(())
    });
    if let Err(e) = saved {
        if let Some(d) = daemon {
            let _ = d.side_vars_move(into, from, &reversed(&undo.daemon_moved));
            let _ = d.side_vars_move(&parking(&undo.id), from, &same_names(&undo.parked));
        }
        return Err(e);
    }
    undo.renames = rename.into_iter().collect();
    Ok(undo)
}

fn reversed(pairs: &[(String, String)]) -> Vec<(String, String)> {
    pairs.iter().map(|(a, b)| (b.clone(), a.clone())).collect()
}

fn same_names(names: &[String]) -> Vec<(String, String)> {
    names.iter().map(|n| (n.clone(), n.clone())).collect()
}

/// Whether [`undo`] can run now: a merge that moved daemon variables needs the daemon.
pub fn can_undo(daemon: Option<&DaemonClient>, undo: &Undo) -> Result<(), ContextError> {
    if daemon.is_none() && (!undo.daemon_moved.is_empty() || !undo.parked.is_empty()) {
        return Err(ContextError::NoDaemon("not connected".into()));
    }
    Ok(())
}

/// Reverses a merge. The source side tab must exist again ([`crate::state::Command::RestoreSideTab`]).
/// The daemon goes first, like the merge; a saved-row failure puts its variables forward again.
pub fn undo(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    undo: &Undo,
) -> Result<(), ContextError> {
    can_undo(daemon, undo)?;
    let back = reversed(&undo.daemon_moved);
    let park = same_names(&undo.parked);
    if let Some(d) = daemon {
        if !back.is_empty() {
            d.side_vars_move(&undo.into, &undo.from, &back)
                .map_err(daemon_err)?;
        }
        if !park.is_empty()
            && let Err(e) = d.side_vars_move(&parking(&undo.id), &undo.from, &park)
        {
            let _ = d.side_vars_move(&undo.from, &undo.into, &undo.daemon_moved);
            return Err(daemon_err(e));
        }
    }
    let saved = db.with(|db| -> Result<(), ContextError> {
        let tx = db.conn().unchecked_transaction()?;
        // Items leave the target before a renamed one takes its old name back: in the target
        // that name belongs to the variable it clashed with.
        for (id, position) in &undo.items {
            tx.execute(
                "UPDATE context_items SET side_tab_id = ?2, position = ?3 WHERE id = ?1",
                params![id, undo.from, position],
            )?;
        }
        for (id, old) in &undo.renamed {
            tx.execute(
                "UPDATE context_items SET name = ?2 WHERE id = ?1",
                params![id, old],
            )?;
        }
        for (table, id) in &undo.moved {
            tx.execute(
                &format!("UPDATE {table} SET side_tab_id = ?2 WHERE id = ?1"),
                params![id, undo.from],
            )?;
        }
        for row in &undo.dropped {
            put_back(db, row)?;
        }
        tx.commit()?;
        Ok(())
    });
    if let (Err(_), Some(d)) = (&saved, daemon) {
        if !park.is_empty() {
            let _ = d.side_vars_move(&undo.from, &parking(&undo.id), &park);
        }
        if !back.is_empty() {
            let _ = d.side_vars_move(&undo.from, &undo.into, &undo.daemon_moved);
        }
    }
    saved
}

/// Ends the undo window: the parked variables are wiped.
pub fn finish(daemon: Option<&DaemonClient>, undo: &Undo) {
    if let (false, Some(d)) = (undo.parked.is_empty(), daemon) {
        let _ = d.side_vars_drop(&parking(&undo.id));
    }
}
