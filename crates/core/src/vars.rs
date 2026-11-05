//! A side tab's variables across both stores (D21, D35): saved ones in SQLite, session and
//! secret ones in the daemon. A name is unique across both. A secret's value never comes back
//! from here, so it cannot reach the UI, a log or the disk.

use crate::context::{self, ContextError, Mode, Variable};
use crate::daemon::DaemonClient;
use crate::db::DbAccess;
use serde::Deserialize;
use terminescu_protocol::VarMode;

/// A variable to add, from the panel, an extract or the CLI.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVariable {
    pub name: String,
    pub value: String,
    pub mode: Mode,
    /// Session and secret only.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// typed, pasted, extracted or auto.
    pub source: String,
    #[serde(default)]
    pub origin_tab_id: Option<String>,
}

/// What to change on a variable; absent fields stay.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableChange {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub mode: Option<Mode>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

fn daemon_mode(mode: Mode) -> Option<VarMode> {
    match mode {
        Mode::Saved => None,
        Mode::Session => Some(VarMode::Session),
        Mode::Secret => Some(VarMode::Secret),
    }
}

fn need(daemon: Option<&DaemonClient>) -> Result<&DaemonClient, ContextError> {
    daemon.ok_or_else(|| ContextError::NoDaemon("not connected".into()))
}

fn daemon_err(e: crate::daemon::DaemonError) -> ContextError {
    ContextError::NoDaemon(e.to_string())
}

/// The daemon's variables of a side tab as context variables. An absent or old daemon has none.
fn in_daemon(daemon: Option<&DaemonClient>, side_tab: &str) -> Result<Vec<Variable>, ContextError> {
    let Some(d) = daemon.filter(|d| d.side_vars) else {
        return Ok(Vec::new());
    };
    let now = crate::now_ms();
    Ok(d.side_var_list(side_tab)
        .map_err(daemon_err)?
        .into_iter()
        .map(|v| {
            let mode = match v.mode {
                VarMode::Session => Mode::Session,
                VarMode::Secret => Mode::Secret,
            };
            Variable {
                id: format!(
                    "{}:{}",
                    if mode == Mode::Secret {
                        "secret"
                    } else {
                        "session"
                    },
                    v.name
                ),
                side_tab_id: side_tab.into(),
                value: (mode == Mode::Session).then_some(v.value),
                name: v.name,
                mode,
                source: None,
                origin_tab_id: None,
                expires_at: v.expires_in_ms.map(|ms| now + ms as i64),
            }
        })
        .collect())
}

/// Saved variables first, in the order they were added, then the daemon's by name.
pub fn list(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    side_tab: &str,
) -> Result<Vec<Variable>, ContextError> {
    let mut all = db.with(|db| context::saved_variables(db, side_tab))?;
    all.extend(in_daemon(daemon, side_tab)?);
    Ok(all)
}

fn find(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    side_tab: &str,
    name: &str,
) -> Result<Option<Variable>, ContextError> {
    if let Some(v) = db.with(|db| context::saved_variable(db, side_tab, name))? {
        return Ok(Some(v));
    }
    Ok(in_daemon(daemon, side_tab)?
        .into_iter()
        .find(|v| v.name == name))
}

/// Adds a variable. A name in use in either store is refused unless `replace`, which then
/// moves the name to the new mode.
pub fn add(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    side_tab: &str,
    new: &NewVariable,
    replace: bool,
) -> Result<Variable, ContextError> {
    if !context::is_variable_name(&new.name) {
        return Err(ContextError::BadName(new.name.clone()));
    }
    if new.value.trim().is_empty() {
        return Err(ContextError::Empty);
    }
    let existing = find(db, daemon, side_tab, &new.name)?;
    if let Some(old) = &existing {
        if !replace {
            return Err(ContextError::NameTaken(side_tab.into(), new.name.clone()));
        }
        if old.mode == Mode::Secret && new.mode != Mode::Secret {
            return Err(ContextError::SecretStays(new.name.clone()));
        }
    }
    let Some(mode) = daemon_mode(new.mode) else {
        let v = db.with(|db| {
            context::put_saved(
                db,
                side_tab,
                &new.name,
                &new.value,
                &new.source,
                new.origin_tab_id.as_deref(),
                true,
            )
        })?;
        if existing.is_some_and(|o| o.mode != Mode::Saved) {
            // The name was the daemon's alone; when the daemon keeps it, take the new row back
            // so one name never lives in both stores.
            let unset = need(daemon)
                .and_then(|d| d.side_var_unset(side_tab, &new.name).map_err(daemon_err));
            if let Err(e) = unset {
                db.with(|db| context::remove_saved(db, side_tab, &new.name))?;
                return Err(e);
            }
        }
        return Ok(v);
    };
    let d = need(daemon)?;
    // Into the daemon first, so a failure leaves the saved copy where it was.
    d.side_var_set(side_tab, &new.name, new.value.clone(), mode, new.ttl_secs)
        .map_err(daemon_err)?;
    db.with(|db| -> Result<(), ContextError> {
        if context::remove_saved(db, side_tab, &new.name)? {
            db.scrub()?;
        }
        Ok(())
    })?;
    find(db, daemon, side_tab, &new.name)?.ok_or_else(|| ContextError::NoVariable(new.name.clone()))
}

/// Removes a variable from whichever store holds it.
pub fn remove(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    side_tab: &str,
    name: &str,
) -> Result<(), ContextError> {
    if db.with(|db| context::remove_saved(db, side_tab, name))? {
        return Ok(());
    }
    if let Some(d) = daemon.filter(|d| d.side_vars) {
        d.side_var_unset(side_tab, name).map_err(daemon_err)?;
    }
    Ok(())
}

/// Changes a variable's value, mode or TTL. A secret stays a secret, and changing its TTL needs
/// its value again, since the daemon never hands a secret back.
pub fn change(
    db: &impl DbAccess,
    daemon: Option<&DaemonClient>,
    side_tab: &str,
    name: &str,
    change: &VariableChange,
) -> Result<Variable, ContextError> {
    let old =
        find(db, daemon, side_tab, name)?.ok_or_else(|| ContextError::NoVariable(name.into()))?;
    let mode = change.mode.unwrap_or(old.mode);
    if old.mode == Mode::Secret && mode != Mode::Secret {
        return Err(ContextError::SecretStays(name.into()));
    }
    let value = match (&change.value, &old.value) {
        (Some(v), _) => v.clone(),
        (None, Some(v)) => v.clone(),
        (None, None) => {
            if change.ttl_secs.is_none() {
                return Ok(old);
            }
            return Err(ContextError::NeedsValue(name.into()));
        }
    };
    let ttl_secs = change.ttl_secs.or_else(|| {
        old.expires_at
            .map(|at| ((at - crate::now_ms()).max(1000) / 1000) as u64)
    });
    add(
        db,
        daemon,
        side_tab,
        &NewVariable {
            name: name.into(),
            value,
            mode,
            ttl_secs: if mode == Mode::Saved { None } else { ttl_secs },
            source: old.source.clone().unwrap_or_else(|| "typed".into()),
            origin_tab_id: old.origin_tab_id.clone(),
        },
        true,
    )
}
