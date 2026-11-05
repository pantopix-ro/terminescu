//! Saved statuses, status rules and the built-in looks in SQLite (task 5.3, D15). A status with
//! no side tab is global; one with a side tab is local to it until "make global".

use crate::db::{Db, DbError};
use crate::state::Id;
use crate::status::{Auto, Look, Rule, RuleKind, StatusError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const BUILTINS_KEY: &str = "builtin_statuses";
/// Longest wait a Claude idle rule takes: one day.
const MAX_IDLE_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedStatus {
    pub id: Id,
    /// Absent for a global status.
    pub side_tab_id: Option<Id>,
    #[serde(flatten)]
    pub look: Look,
}

/// Everything the status editor and Settings › Statuses show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusConfig {
    /// Global statuses in their order, then local ones.
    pub statuses: Vec<SavedStatus>,
    pub rules: Vec<Rule>,
    /// Renamed or recolored built-ins; a built-in left out keeps its default look.
    pub builtins: BTreeMap<Auto, Look>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Look(#[from] StatusError),
    #[error("no status {0}")]
    UnknownStatus(Id),
    #[error("a rule needs a command pattern")]
    EmptyPattern,
    #[error("an idle rule waits 1 to {MAX_IDLE_MINUTES} minutes")]
    BadMinutes,
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Db(e.into())
    }
}

type Result<T> = std::result::Result<T, StoreError>;

fn clean(look: &Look) -> Result<Look> {
    let look = Look {
        label: look.label.trim().to_string(),
        ..look.clone()
    };
    look.check()?;
    Ok(look)
}

pub fn statuses(db: &Db) -> Result<Vec<SavedStatus>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, side_tab_id, label, accent, background FROM statuses WHERE owner_id = ?1
         ORDER BY side_tab_id IS NOT NULL, position, created_at, id",
    )?;
    let rows = stmt.query_map([owner], |r| {
        Ok(SavedStatus {
            id: r.get(0)?,
            side_tab_id: r.get(1)?,
            look: Look {
                label: r.get(2)?,
                accent: r.get(3)?,
                background: r.get(4)?,
            },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn status(db: &Db, id: &str) -> Result<SavedStatus> {
    statuses(db)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| StoreError::UnknownStatus(id.into()))
}

/// Saves a status on `side_tab`, or globally with `None`, at the end of its list.
/// The global statuses a new database starts with (task 8.21), as (label, accent).
pub const DEFAULT_STATUSES: &[(&str, &str)] = &[
    ("In progress", "blue"),
    ("Review", "magenta"),
    ("Testing", "cyan"),
    ("Blocked", "red"),
    ("Done", "green"),
];

const SEEDED: &str = "statuses.seeded";

/// Adds [`DEFAULT_STATUSES`] once per database: a label already saved is skipped, and one the
/// person deleted later does not come back. Returns how many were added.
pub fn seed_defaults(db: &Db) -> Result<usize> {
    let owner = db.local_owner()?;
    let done = db
        .conn()
        .query_row(
            "SELECT 1 FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, SEEDED],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if done {
        return Ok(0);
    }
    let have = statuses(db)?;
    let mut added = 0;
    for (label, accent) in DEFAULT_STATUSES {
        if have
            .iter()
            .any(|s| s.look.label.eq_ignore_ascii_case(label))
        {
            continue;
        }
        let look = Look {
            label: (*label).into(),
            accent: Some((*accent).into()),
            background: None,
        };
        add_status(db, None, &look)?;
        added += 1;
    }
    db.conn().execute(
        "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, '1')",
        params![owner, SEEDED],
    )?;
    Ok(added)
}

pub fn add_status(db: &Db, side_tab: Option<&str>, look: &Look) -> Result<SavedStatus> {
    let look = clean(look)?;
    let owner = db.local_owner()?;
    let id = crate::new_id();
    db.conn().execute(
        "INSERT INTO statuses (id, owner_id, side_tab_id, label, accent, background, position, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6,
           (SELECT COALESCE(MAX(position), -1) + 1 FROM statuses WHERE owner_id = ?2), ?7)",
        params![id, owner, side_tab, look.label, look.accent, look.background, crate::now_ms()],
    )?;
    Ok(SavedStatus {
        id,
        side_tab_id: side_tab.map(String::from),
        look,
    })
}

pub fn update_status(db: &Db, id: &str, look: &Look) -> Result<SavedStatus> {
    let look = clean(look)?;
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "UPDATE statuses SET label = ?3, accent = ?4, background = ?5 WHERE owner_id = ?1 AND id = ?2",
        params![owner, id, look.label, look.accent, look.background],
    )?;
    if n == 0 {
        return Err(StoreError::UnknownStatus(id.into()));
    }
    status(db, id)
}

/// Deletes a status and the rules that set it. Returns whether it existed.
pub fn delete_status(db: &Db, id: &str) -> Result<bool> {
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "DELETE FROM statuses WHERE owner_id = ?1 AND id = ?2",
        params![owner, id],
    )?;
    Ok(n > 0)
}

pub fn make_global(db: &Db, id: &str) -> Result<SavedStatus> {
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "UPDATE statuses SET side_tab_id = NULL,
           position = (SELECT COALESCE(MAX(position), -1) + 1 FROM statuses WHERE owner_id = ?1)
         WHERE owner_id = ?1 AND id = ?2",
        params![owner, id],
    )?;
    if n == 0 {
        return Err(StoreError::UnknownStatus(id.into()));
    }
    status(db, id)
}

/// Puts the global statuses in the order of `ids`; ids not listed keep their place after them.
pub fn reorder(db: &Db, ids: &[Id]) -> Result<()> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE statuses SET position = ?3 WHERE owner_id = ?1 AND id = ?2",
            params![owner, id, i as i64],
        )?;
    }
    tx.execute(
        "UPDATE statuses SET position = position + ?2
         WHERE owner_id = ?1 AND id NOT IN (SELECT value FROM json_each(?3))",
        params![
            owner,
            ids.len() as i64,
            serde_json::to_string(ids).expect("ids serialize")
        ],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn rules(db: &Db) -> Result<Vec<Rule>> {
    let owner = db.local_owner()?;
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT r.id, r.status_id, s.label, s.accent, s.background, r.kind, r.params_json, r.enabled
         FROM status_rules r JOIN statuses s ON s.id = r.status_id
         WHERE r.owner_id = ?1 ORDER BY r.priority, r.rowid",
    )?;
    let rows = stmt.query_map([owner], |r| {
        let kind: String = r.get(5)?;
        let params: String = r.get(6)?;
        Ok((
            Rule {
                id: r.get(0)?,
                status_id: r.get(1)?,
                look: Look {
                    label: r.get(2)?,
                    accent: r.get(3)?,
                    background: r.get(4)?,
                },
                kind: RuleKind::ClaudeIdle { minutes: 0 },
                enabled: r.get(7)?,
            },
            kind,
            params,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (mut rule, kind, params) = row?;
        // The kind and its parameters travel as one tagged object; a row that no longer
        // parses is skipped rather than failing every rule.
        let mut obj: serde_json::Value =
            serde_json::from_str(&params).unwrap_or(serde_json::json!({}));
        obj["kind"] = serde_json::Value::String(kind);
        if let Ok(k) = serde_json::from_value::<RuleKind>(obj) {
            rule.kind = k;
            out.push(rule);
        }
    }
    Ok(out)
}

fn check_kind(kind: &RuleKind) -> Result<RuleKind> {
    match kind {
        RuleKind::ExitCode { pattern } if pattern.trim().is_empty() => {
            Err(StoreError::EmptyPattern)
        }
        RuleKind::ExitCode { pattern } => Ok(RuleKind::ExitCode {
            pattern: pattern.trim().into(),
        }),
        RuleKind::ClaudeIdle { minutes } if !(1..=MAX_IDLE_MINUTES).contains(minutes) => {
            Err(StoreError::BadMinutes)
        }
        k => Ok(k.clone()),
    }
}

pub fn add_rule(db: &Db, status_id: &str, kind: &RuleKind) -> Result<Rule> {
    let kind = check_kind(kind)?;
    let saved = status(db, status_id)?;
    let owner = db.local_owner()?;
    let id = crate::new_id();
    let mut params_json = serde_json::to_value(&kind).expect("a rule kind serializes");
    let name = params_json["kind"].as_str().unwrap_or_default().to_string();
    params_json
        .as_object_mut()
        .expect("a tagged enum is an object")
        .remove("kind");
    db.conn().execute(
        "INSERT INTO status_rules (id, owner_id, status_id, kind, params_json, priority, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5,
           (SELECT COALESCE(MAX(priority), -1) + 1 FROM status_rules WHERE owner_id = ?2), 1)",
        params![id, owner, status_id, name, params_json.to_string()],
    )?;
    Ok(Rule {
        id,
        status_id: status_id.into(),
        look: saved.look,
        kind,
        enabled: true,
    })
}

pub fn set_rule_enabled(db: &Db, id: &str, enabled: bool) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "UPDATE status_rules SET enabled = ?3 WHERE owner_id = ?1 AND id = ?2",
        params![owner, id, enabled],
    )?;
    Ok(())
}

pub fn delete_rule(db: &Db, id: &str) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "DELETE FROM status_rules WHERE owner_id = ?1 AND id = ?2",
        params![owner, id],
    )?;
    Ok(())
}

pub fn builtins(db: &Db) -> Result<BTreeMap<Auto, Look>> {
    let owner = db.local_owner()?;
    let json: Option<String> = db
        .conn()
        .query_row(
            "SELECT value FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, BUILTINS_KEY],
            |r| r.get(0),
        )
        .optional()?;
    let mut map: BTreeMap<Auto, Look> = json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    map.retain(|_, l| l.check().is_ok());
    Ok(map)
}

/// Renames or recolors a built-in status; `None` gives it back its default look.
pub fn set_builtin(db: &Db, auto: Auto, look: Option<&Look>) -> Result<()> {
    let mut map = builtins(db)?;
    match look {
        Some(l) => {
            let l = clean(l)?;
            // Waiting keeps amber: it is the needs-you signal (D16).
            if auto == Auto::Waiting && (l.accent.is_some() || l.background.is_some()) {
                return Err(StoreError::Look(StatusError::BadAccent(
                    "anything but amber on Waiting".into(),
                )));
            }
            map.insert(auto, l)
        }
        None => map.remove(&auto),
    };
    let owner = db.local_owner()?;
    db.conn().execute(
        "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, ?3)
         ON CONFLICT (owner_id, key) DO UPDATE SET value = excluded.value",
        params![
            owner,
            BUILTINS_KEY,
            serde_json::to_string(&map).expect("looks serialize")
        ],
    )?;
    Ok(())
}

pub fn config(db: &Db) -> Result<StatusConfig> {
    Ok(StatusConfig {
        statuses: statuses(db)?,
        rules: rules(db)?,
        builtins: builtins(db)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(label: &str, accent: Option<&str>) -> Look {
        Look {
            label: label.into(),
            accent: accent.map(String::from),
            background: None,
        }
    }

    #[test]
    fn defaults_come_once_and_skip_labels_already_saved() {
        let db = Db::open_in_memory().unwrap();
        add_status(&db, None, &look("review", Some("cyan"))).unwrap();
        assert_eq!(seed_defaults(&db).unwrap(), DEFAULT_STATUSES.len() - 1);
        let all = statuses(&db).unwrap();
        assert_eq!(all.len(), DEFAULT_STATUSES.len());
        assert!(all.iter().all(|s| s.side_tab_id.is_none()));
        let blocked = all.iter().find(|s| s.look.label == "Blocked").unwrap();
        delete_status(&db, &blocked.id).unwrap();
        assert_eq!(seed_defaults(&db).unwrap(), 0);
        assert_eq!(statuses(&db).unwrap().len(), DEFAULT_STATUSES.len() - 1);
    }

    #[test]
    fn statuses_rules_and_builtins_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let review = add_status(&db, None, &look(" Review ", Some("cyan"))).unwrap();
        assert_eq!(review.look.label, "Review");
        let red = add_status(&db, None, &look("Tests red", Some("red"))).unwrap();
        reorder(&db, &[red.id.clone(), review.id.clone()]).unwrap();
        let labels: Vec<String> = statuses(&db)
            .unwrap()
            .into_iter()
            .map(|s| s.look.label)
            .collect();
        assert_eq!(labels, ["Tests red", "Review"]);

        let rule = add_rule(
            &db,
            &red.id,
            &RuleKind::ExitCode {
                pattern: " pnpm test* ".into(),
            },
        )
        .unwrap();
        assert_eq!(rules(&db).unwrap(), vec![rule.clone()]);
        assert!(matches!(
            add_rule(&db, &red.id, &RuleKind::ClaudeIdle { minutes: 0 }),
            Err(StoreError::BadMinutes)
        ));
        set_rule_enabled(&db, &rule.id, false).unwrap();
        assert!(!rules(&db).unwrap()[0].enabled);

        update_status(&db, &red.id, &look("Red", Some("magenta"))).unwrap();
        assert_eq!(rules(&db).unwrap()[0].look.label, "Red");
        assert!(delete_status(&db, &red.id).unwrap());
        assert!(rules(&db).unwrap().is_empty(), "its rules go with it");

        set_builtin(&db, Auto::Running, Some(&look("Busy", Some("green")))).unwrap();
        assert!(set_builtin(&db, Auto::Waiting, Some(&look("Ask", Some("red")))).is_err());
        assert_eq!(builtins(&db).unwrap()[&Auto::Running].label, "Busy");
        set_builtin(&db, Auto::Running, None).unwrap();
        assert!(builtins(&db).unwrap().is_empty());
        assert!(add_status(&db, None, &look("x", Some("amber"))).is_err());
    }
}
