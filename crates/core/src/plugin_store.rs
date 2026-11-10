//! Plugin storage (add-plugin-architecture D10): each plugin's settings, its on/off switch among
//! them, and per-row data that the database deletes with its row.

use crate::db::{Db, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use std::collections::HashMap;

/// The setting that holds a plugin's switch.
pub const ENABLED: &str = "enabled";

fn parse(json: Option<String>) -> Option<Value> {
    json.and_then(|j| serde_json::from_str(&j).ok())
}

pub fn setting(db: &Db, plugin: &str, key: &str) -> Result<Option<Value>> {
    let owner = db.local_owner()?;
    let json = db
        .conn()
        .query_row(
            "SELECT value_json FROM plugin_settings
             WHERE owner_id = ?1 AND plugin_id = ?2 AND key = ?3",
            params![owner, plugin, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(parse(json))
}

/// Sets a setting; `None` removes it.
pub fn set_setting(db: &Db, plugin: &str, key: &str, value: Option<&Value>) -> Result<()> {
    let owner = db.local_owner()?;
    match value {
        Some(v) => db.conn().execute(
            "INSERT INTO plugin_settings (owner_id, plugin_id, key, value_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (owner_id, plugin_id, key) DO UPDATE SET value_json = excluded.value_json",
            params![owner, plugin, key, v.to_string()],
        )?,
        None => db.conn().execute(
            "DELETE FROM plugin_settings WHERE owner_id = ?1 AND plugin_id = ?2 AND key = ?3",
            params![owner, plugin, key],
        )?,
    };
    Ok(())
}

/// The saved switches, by plugin. A plugin with no saved switch is on.
pub fn switches(db: &Db) -> Result<HashMap<String, bool>> {
    let owner = db.local_owner()?;
    let mut stmt = db.conn().prepare(
        "SELECT plugin_id, value_json FROM plugin_settings WHERE owner_id = ?1 AND key = ?2",
    )?;
    let rows = stmt
        .query_map(params![owner, ENABLED], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .map(|(id, json)| (id, json.trim() != "false"))
        .collect())
}

pub fn set_enabled(db: &Db, plugin: &str, on: bool) -> Result<()> {
    set_setting(db, plugin, ENABLED, Some(&Value::Bool(on)))
}

pub fn row_data(db: &Db, side_tab: &str, plugin: &str, key: &str) -> Result<Option<Value>> {
    let json = db
        .conn()
        .query_row(
            "SELECT value_json FROM plugin_side_tab_data
             WHERE side_tab_id = ?1 AND plugin_id = ?2 AND key = ?3",
            params![side_tab, plugin, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(parse(json))
}

/// Every key a plugin keeps on a row.
pub fn row_data_all(
    db: &Db,
    side_tab: &str,
    plugin: &str,
) -> Result<std::collections::BTreeMap<String, Value>> {
    let mut stmt = db.conn().prepare(
        "SELECT key, value_json FROM plugin_side_tab_data WHERE side_tab_id = ?1 AND plugin_id = ?2",
    )?;
    let rows = stmt
        .query_map(params![side_tab, plugin], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(k, json)| parse(json).map(|v| (k, v)))
        .collect())
}

/// Sets a row's data; `None` removes it. A row that is not saved yet fails on its foreign key.
pub fn set_row_data(
    db: &Db,
    side_tab: &str,
    plugin: &str,
    key: &str,
    value: Option<&Value>,
) -> Result<()> {
    let owner = db.local_owner()?;
    match value {
        Some(v) => db.conn().execute(
            "INSERT INTO plugin_side_tab_data (owner_id, side_tab_id, plugin_id, key, value_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (side_tab_id, plugin_id, key) DO UPDATE SET value_json = excluded.value_json",
            params![owner, side_tab, plugin, key, v.to_string()],
        )?,
        None => db.conn().execute(
            "DELETE FROM plugin_side_tab_data WHERE side_tab_id = ?1 AND plugin_id = ?2 AND key = ?3",
            params![side_tab, plugin, key],
        )?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist;
    use crate::state::{Command, Core, Workspace};
    use serde_json::json;

    #[test]
    fn plugin_settings_persist() {
        let dir = std::env::temp_dir().join(format!("terminescu-plugins-{}", crate::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        {
            let db = Db::open(&path).unwrap();
            assert!(switches(&db).unwrap().is_empty(), "no switch means on");
            set_enabled(&db, "git", false).unwrap();
            set_setting(&db, "git", "worktreeRoot", Some(&json!("~/worktrees"))).unwrap();
            set_setting(&db, "claude", "x", Some(&json!(1))).unwrap();
            set_setting(&db, "claude", "x", None).unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(
            switches(&db).unwrap(),
            HashMap::from([("git".into(), false)])
        );
        assert_eq!(
            setting(&db, "git", "worktreeRoot").unwrap(),
            Some(json!("~/worktrees"))
        );
        assert_eq!(setting(&db, "claude", "x").unwrap(), None);
        set_enabled(&db, "git", true).unwrap();
        assert!(switches(&db).unwrap()["git"]);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn row_data_goes_with_its_row() {
        let db = Db::open_in_memory().unwrap();
        let mut n = 0;
        let mut c = Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        );
        c.handle(Command::CreateWindow).unwrap();
        let window = c.workspace().windows[0].id.clone();
        for _ in 0..2 {
            c.handle(Command::CreateSideTab {
                window_id: window.clone(),
                index: None,
            })
            .unwrap();
        }
        let save = |ws: &Workspace| {
            persist::save(
                &db,
                &persist::Saved {
                    workspace: ws.clone(),
                    ..Default::default()
                },
            )
            .unwrap()
        };
        save(c.workspace());
        let rows: Vec<String> = c.workspace().windows[0]
            .side_tabs
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let (gone, kept) = (&rows[0], &rows[1]);
        set_row_data(
            &db,
            gone,
            "git",
            "worktree",
            Some(&json!({"path": "/tmp/a"})),
        )
        .unwrap();
        set_row_data(
            &db,
            kept,
            "git",
            "worktree",
            Some(&json!({"path": "/tmp/b"})),
        )
        .unwrap();
        assert!(set_row_data(&db, "nope", "git", "k", Some(&json!(1))).is_err());

        c.handle(Command::CloseSideTab {
            side_tab_id: gone.clone(),
        })
        .unwrap();
        save(c.workspace());
        assert_eq!(row_data(&db, gone, "git", "worktree").unwrap(), None);
        assert_eq!(
            row_data(&db, kept, "git", "worktree").unwrap(),
            Some(json!({"path": "/tmp/b"}))
        );
    }
}
