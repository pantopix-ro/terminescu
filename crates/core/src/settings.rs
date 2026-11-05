//! App settings that are not statuses, commands or attention signals (D25): the defaults an
//! extracted variable starts with, and the opt-in editor setting for the shell (D27). Kept as
//! one JSON value in `app_state`.

use crate::context::Mode;
use crate::db::{Db, Result};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const KEY: &str = "app.settings";
/// Longest TTL a default may hold: 30 days.
pub const MAX_TTL_SECS: u64 = 30 * 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The mode an extracted variable starts with (Settings › Variables).
    pub var_mode: Mode,
    /// The TTL a session or secret variable starts with; none keeps it until the side tab ends.
    pub var_ttl_secs: Option<u64>,
    /// `VISUAL` and `EDITOR` point at `terminescu edit --wait` (Settings › Shell, D27).
    pub editor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            var_mode: Mode::Saved,
            var_ttl_secs: None,
            editor: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("a TTL runs from 1 second to 30 days")]
    BadTtl,
    #[error(transparent)]
    Db(#[from] crate::db::DbError),
}

pub fn get(db: &Db) -> Result<Settings> {
    Ok(read(db, KEY)?.unwrap_or_default())
}

pub fn set(db: &Db, s: Settings) -> std::result::Result<Settings, SettingsError> {
    if s.var_ttl_secs.is_some_and(|t| t == 0 || t > MAX_TTL_SECS) {
        return Err(SettingsError::BadTtl);
    }
    write(db, KEY, &s)?;
    Ok(s)
}

/// One JSON value from `app_state`; a value that no longer parses reads as absent.
pub(crate) fn read<T: DeserializeOwned>(db: &Db, key: &str) -> Result<Option<T>> {
    let owner = db.local_owner()?;
    let json: Option<String> = db
        .conn()
        .query_row(
            "SELECT value FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

pub(crate) fn write<T: Serialize>(db: &Db, key: &str, value: &T) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, ?3)
         ON CONFLICT (owner_id, key) DO UPDATE SET value = excluded.value",
        params![
            owner,
            key,
            serde_json::to_string(value).expect("settings serialize")
        ],
    )?;
    Ok(())
}
