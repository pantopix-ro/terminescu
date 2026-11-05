//! The layout in SQLite (task 3.3): windows, side tabs, tabs with their split trees, and per
//! pane the daemon pane, the last cwd and the Claude session that ran there (D18, D23).
//!
//! `save` writes the whole layout in one transaction. It updates rows in place and deletes only
//! rows that left the layout, so a side tab's context items survive every save. A tab that
//! changed side tab takes the items it made along (D5), before its old side tab may go.

use crate::db::{Db, Result};
use crate::layout::Layout;
use crate::state::{Id, SideTab, Tab, Window, Workspace};
use crate::status::ManualStatus;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;
use terminescu_protocol::PaneId;

const INSTANCE_KEY: &str = "ptyd_instance";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneRecord {
    /// The daemon pane, valid only while the daemon instance matches.
    pub daemon: Option<PaneId>,
    pub cwd: Option<String>,
    /// The Claude session running in the pane, from its hooks.
    pub session: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub workspace: Workspace,
    pub panes: HashMap<Id, PaneRecord>,
    /// The daemon instance the daemon pane ids belong to.
    pub instance: Option<String>,
}

fn status_json(status: &Option<ManualStatus>) -> Option<String> {
    status
        .as_ref()
        .map(|s| serde_json::to_string(s).expect("a status serializes"))
}

/// A saved icon, dropped when a later build no longer lists it.
fn icon_of(icon: Option<String>) -> Option<String> {
    icon.filter(|i| crate::state::ICONS.contains(&i.as_str()))
}

/// A saved neon accent, dropped when a later build no longer lists it.
fn accent_of(accent: Option<String>) -> Option<String> {
    accent.filter(|a| crate::state::NEONS.contains(&a.as_str()))
}

/// A saved manual status, dropped when it no longer parses or checks.
fn status_of(json: Option<String>) -> Option<ManualStatus> {
    serde_json::from_str::<ManualStatus>(&json?)
        .ok()
        .filter(|s| s.look.check().is_ok())
}

pub fn save(db: &Db, saved: &Saved) -> Result<()> {
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let was: HashMap<String, String> = {
        let mut st = tx.prepare("SELECT id, side_tab_id FROM tabs WHERE owner_id = ?1")?;
        st.query_map(params![owner], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut moved: Vec<(&str, &str, &str)> = Vec::new();
    let mut ids: [Vec<&str>; 4] = Default::default();
    for (wi, w) in saved.workspace.windows.iter().enumerate() {
        tx.execute(
            "INSERT INTO windows (id, owner_id, position, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (id) DO UPDATE SET position = excluded.position",
            params![w.id, owner, wi as i64, now],
        )?;
        ids[0].push(&w.id);
        for (si, s) in w.side_tabs.iter().enumerate() {
            tx.execute(
                "INSERT INTO side_tabs (id, owner_id, window_id, position, title, status_json, created_at, icon, accent)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?7, ?6, ?8, ?9)
                 ON CONFLICT (id) DO UPDATE SET window_id = excluded.window_id,
                   position = excluded.position, title = excluded.title,
                   status_json = excluded.status_json, icon = excluded.icon, accent = excluded.accent",
                params![s.id, owner, w.id, si as i64, s.title, now, status_json(&s.status), s.icon, s.accent],
            )?;
            ids[1].push(&s.id);
            for (ti, t) in s.tabs.iter().enumerate() {
                let layout = serde_json::to_string(&t.layout).expect("a layout serializes");
                tx.execute(
                    "INSERT INTO tabs (id, owner_id, side_tab_id, position, title, layout_json, status_json, created_at, icon)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?8, ?7, ?9)
                     ON CONFLICT (id) DO UPDATE SET side_tab_id = excluded.side_tab_id,
                       position = excluded.position, title = excluded.title,
                       layout_json = excluded.layout_json, status_json = excluded.status_json,
                       icon = excluded.icon",
                    params![t.id, owner, s.id, ti as i64, t.title, layout, now, status_json(&t.status), t.icon],
                )?;
                ids[2].push(&t.id);
                if let Some(old) = was.get(&t.id).filter(|old| **old != s.id) {
                    moved.push((&t.id, old, &s.id));
                }
                let terminals = t.layout.terminals();
                let docs = t.layout.docs();
                for p in t.layout.panes() {
                    let rec = saved.panes.get(p).cloned().unwrap_or_default();
                    let doc = docs.iter().find(|(pane, _)| *pane == p).map(|(_, d)| *d);
                    let kind = if terminals.contains(&p) {
                        "terminal"
                    } else if doc.is_some() {
                        "text"
                    } else {
                        "transcript"
                    };
                    tx.execute(
                        "INSERT INTO panes (id, owner_id, tab_id, kind, daemon_pane_id, cwd, session_id, document_id, created_at)
                         VALUES (?1, ?2, ?3, ?8, ?4, ?5, ?6, ?9, ?7)
                         ON CONFLICT (id) DO UPDATE SET tab_id = excluded.tab_id, kind = excluded.kind,
                           daemon_pane_id = excluded.daemon_pane_id, cwd = excluded.cwd,
                           session_id = excluded.session_id, document_id = excluded.document_id",
                        params![p, owner, t.id, rec.daemon.map(|d| d as i64), rec.cwd, rec.session, now, kind, doc],
                    )?;
                    ids[3].push(p);
                }
            }
        }
    }
    for (tab, from, to) in moved {
        follow(&tx, &owner, tab, from, to)?;
    }
    // Children first; cascades would cover it, but explicit deletes keep the order obvious.
    for (table, keep) in [
        ("panes", &ids[3]),
        ("tabs", &ids[2]),
        ("side_tabs", &ids[1]),
        ("windows", &ids[0]),
    ] {
        let keep = serde_json::to_string(keep).expect("ids serialize");
        tx.execute(
            &format!(
                "DELETE FROM {table} WHERE owner_id = ?1 AND id NOT IN (SELECT value FROM json_each(?2))"
            ),
            params![owner, keep],
        )?;
    }
    match &saved.instance {
        Some(instance) => tx.execute(
            "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (owner_id, key) DO UPDATE SET value = excluded.value",
            params![owner, INSTANCE_KEY, instance],
        )?,
        None => tx.execute(
            "DELETE FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, INSTANCE_KEY],
        )?,
    };
    tx.commit()?;
    Ok(())
}

/// Moves what a tab made (extracted and auto items, pins, touched files) to the side tab it
/// moved to (D5). Typed and pasted items stay. A variable or file the target already has by
/// that name or path stays behind too.
fn follow(
    tx: &rusqlite::Transaction<'_>,
    owner: &str,
    tab: &str,
    from: &str,
    to: &str,
) -> Result<()> {
    tx.execute(
        "UPDATE OR IGNORE context_items SET side_tab_id = ?4,
           position = position + (SELECT COALESCE(MAX(position) + 1, 0) FROM context_items
                                   WHERE side_tab_id = ?4)
         WHERE owner_id = ?1 AND origin_tab_id = ?2 AND side_tab_id = ?3
           AND source IN ('extracted', 'auto')",
        params![owner, tab, from, to],
    )?;
    tx.execute(
        "UPDATE pins SET side_tab_id = ?4
         WHERE owner_id = ?1 AND origin_tab_id = ?2 AND side_tab_id = ?3",
        params![owner, tab, from, to],
    )?;
    tx.execute(
        "UPDATE OR IGNORE context_files SET side_tab_id = ?4
         WHERE owner_id = ?1 AND origin_tab_id = ?2 AND side_tab_id = ?3",
        params![owner, tab, from, to],
    )?;
    Ok(())
}

/// The saved layout, or `None` when nothing was saved yet. A tab whose layout no longer parses
/// gets one pane, so a bad row costs that tab's split, not the start.
pub fn load(db: &Db) -> Result<Option<Saved>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut windows: Vec<Window> = conn
        .prepare("SELECT id FROM windows WHERE owner_id = ?1 ORDER BY position")?
        .query_map([&owner], |r| {
            Ok(Window {
                id: r.get(0)?,
                side_tabs: vec![],
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    if windows.is_empty() {
        return Ok(None);
    }
    let mut side_tabs: Vec<(Id, SideTab)> = conn
        .prepare(
            "SELECT window_id, id, title, status_json, icon, accent FROM side_tabs WHERE owner_id = ?1 ORDER BY position",
        )?
        .query_map([&owner], |r| {
            Ok((
                r.get(0)?,
                SideTab {
                    id: r.get(1)?,
                    title: r.get(2)?,
                    tabs: vec![],
                    status: status_of(r.get(3)?),
                    icon: icon_of(r.get(4)?),
                    accent: accent_of(r.get(5)?),
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    type TabRow = (
        Id,
        Id,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    );
    let tabs: Vec<TabRow> = conn
        .prepare("SELECT side_tab_id, id, title, layout_json, status_json, icon FROM tabs WHERE owner_id = ?1 ORDER BY position")?
        .query_map([&owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut panes: HashMap<Id, PaneRecord> = conn
        .prepare("SELECT id, daemon_pane_id, cwd, session_id FROM panes WHERE owner_id = ?1")?
        .query_map([&owner], |r| {
            Ok((
                r.get::<_, Id>(0)?,
                PaneRecord {
                    daemon: r.get::<_, Option<i64>>(1)?.map(|d| d as PaneId),
                    cwd: r.get(2)?,
                    session: r.get(3)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (side_tab_id, id, title, layout_json, status_json, icon) in tabs {
        let mut layout = serde_json::from_str::<Layout>(&layout_json).unwrap_or_else(|e| {
            eprintln!(
                "terminescu: tab {id} has an unreadable layout, it restarts with one pane: {e}"
            );
            Layout::pane(crate::new_id())
        });
        // A resume offer comes back only with an id that is safe to type (D23).
        let panes: Vec<String> = layout.panes().into_iter().cloned().collect();
        for pane in panes {
            if let Some(Layout::Pane { resume, .. }) = layout.leaf_mut(&pane)
                && resume
                    .as_deref()
                    .is_some_and(|r| !crate::runtime::is_session_id(r))
            {
                *resume = None;
            }
            // A transcript pane with a bad id goes; one with a good id stays shell-less.
            if let Some(Layout::Pane {
                transcript: Some(t),
                ..
            }) = layout.leaf_mut(&pane)
                && !crate::runtime::is_session_id(t)
            {
                layout = layout
                    .remove(&pane)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| Layout::pane(crate::new_id()));
            }
        }
        if !layout.holds_work() {
            layout = Layout::pane(crate::new_id());
        }
        if let Some((_, s)) = side_tabs.iter_mut().find(|(_, s)| s.id == side_tab_id) {
            s.tabs.push(Tab {
                id,
                title,
                layout,
                status: status_of(status_json),
                icon: icon_of(icon),
            });
        }
    }
    for (window_id, s) in side_tabs {
        // A side tab left without tabs would be a sidebar row with nothing behind it.
        if s.tabs.is_empty() {
            continue;
        }
        if let Some(w) = windows.iter_mut().find(|w| w.id == window_id) {
            w.side_tabs.push(s);
        }
    }
    let workspace = Workspace { windows };
    let live: Vec<Id> = workspace.pane_sides().into_iter().map(|(p, _)| p).collect();
    panes.retain(|id, _| live.contains(id));
    let instance = conn
        .query_row(
            "SELECT value FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, INSTANCE_KEY],
            |r| r.get(0),
        )
        .optional()?;
    Ok(Some(Saved {
        workspace,
        panes,
        instance,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Dir;

    fn sample() -> Saved {
        let layout = Layout::pane("p1")
            .split("p1", Dir::Row, Layout::pane("p2"), "sp1".into())
            .unwrap();
        let workspace = Workspace {
            windows: vec![Window {
                id: "w1".into(),
                side_tabs: vec![
                    SideTab {
                        accent: None,
                        status: None,
                        icon: None,
                        id: "s1".into(),
                        title: Some("PROJ-482".into()),
                        tabs: vec![
                            Tab {
                                status: None,
                                icon: None,
                                id: "t1".into(),
                                title: None,
                                layout,
                            },
                            Tab {
                                status: None,
                                icon: None,
                                id: "t2".into(),
                                title: Some("dev server".into()),
                                layout: Layout::pane("p3"),
                            },
                        ],
                    },
                    SideTab {
                        accent: None,
                        status: None,
                        icon: None,
                        id: "s2".into(),
                        title: None,
                        tabs: vec![Tab {
                            status: None,
                            icon: None,
                            id: "t3".into(),
                            title: None,
                            layout: Layout::pane("p4"),
                        }],
                    },
                ],
            }],
        };
        // Every pane gets a row on save, so every pane has a record on load.
        let mut panes: HashMap<Id, PaneRecord> = ["p2", "p4"]
            .into_iter()
            .map(|p| (p.into(), PaneRecord::default()))
            .collect();
        panes.insert(
            "p1".into(),
            PaneRecord {
                daemon: Some(7),
                cwd: Some("/tmp/acme-api".into()),
                session: Some("0b5c-example".into()),
            },
        );
        panes.insert(
            "p3".into(),
            PaneRecord {
                daemon: Some(9),
                cwd: None,
                session: None,
            },
        );
        Saved {
            workspace,
            panes,
            instance: Some("inst-1".into()),
        }
    }

    #[test]
    fn nothing_saved_loads_as_none() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(load(&db).unwrap(), None);
    }

    #[test]
    fn a_saved_layout_loads_back_the_same() {
        let db = Db::open_in_memory().unwrap();
        let saved = sample();
        save(&db, &saved).unwrap();
        assert_eq!(load(&db).unwrap(), Some(saved));
    }

    #[test]
    fn icons_load_back_and_unknown_ones_drop() {
        let db = Db::open_in_memory().unwrap();
        let mut saved = sample();
        let side = &mut saved.workspace.windows[0].side_tabs[0];
        side.icon = Some("pin".into());
        side.accent = Some("lime".into());
        side.tabs[0].icon = Some("doc".into());
        save(&db, &saved).unwrap();
        assert_eq!(load(&db).unwrap(), Some(saved.clone()));

        db.conn()
            .execute("UPDATE tabs SET icon = 'rocket'", [])
            .unwrap();
        let loaded = load(&db).unwrap().unwrap();
        assert_eq!(loaded.workspace.windows[0].side_tabs[0].tabs[0].icon, None);
        assert_eq!(
            loaded.workspace.windows[0].side_tabs[0].accent.as_deref(),
            Some("lime")
        );

        db.conn()
            .execute("UPDATE side_tabs SET accent = 'amber'", [])
            .unwrap();
        let loaded = load(&db).unwrap().unwrap();
        assert_eq!(loaded.workspace.windows[0].side_tabs[0].accent, None);
    }

    #[test]
    fn a_later_save_moves_rows_and_keeps_context_items() {
        let db = Db::open_in_memory().unwrap();
        let mut saved = sample();
        save(&db, &saved).unwrap();
        let owner = db.local_owner().unwrap();
        db.conn()
            .execute(
                "INSERT INTO context_items (id, owner_id, side_tab_id, kind, name, value, position, created_at, updated_at)
                 VALUES ('c1', ?1, 's1', 'variable', 'branch', 'feature/x', 0, 0, 0)",
                [&owner],
            )
            .unwrap();
        // t2 moves to s2, s2's own tab closes, the instance goes away.
        let w = &mut saved.workspace.windows[0];
        let t2 = w.side_tabs[0].tabs.remove(1);
        w.side_tabs[1].tabs = vec![t2];
        saved.panes.remove("p4");
        saved.instance = None;
        save(&db, &saved).unwrap();
        assert_eq!(load(&db).unwrap(), Some(saved.clone()));
        let items: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM context_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(items, 1, "side tab s1 lives on, so does its context");
        // Closing s1 takes its context with it.
        saved.workspace.windows[0].side_tabs.remove(0);
        save(&db, &saved).unwrap();
        let items: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM context_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(items, 0);
    }
}
