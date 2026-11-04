//! SQLite with embedded migrations (D18). The schema version lives in `PRAGMA user_version`.

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

const MIGRATIONS: &[(u32, &str)] = &[(1, include_str!("../migrations/0001_init.sql"))];

pub const SCHEMA_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].0;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("database schema {found} is newer than this build knows ({known}); update Terminescu")]
    SchemaTooNew { found: u32, known: u32 },
}

pub type Result<T> = std::result::Result<T, DbError>;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The one local owner of v1, created on first call (D18).
    pub fn local_owner(&self) -> Result<String> {
        let found: Option<String> = self
            .conn
            .query_row("SELECT id FROM owners WHERE is_local = 1", [], |r| r.get(0))
            .optional()?;
        if let Some(id) = found {
            return Ok(id);
        }
        let id = crate::new_id();
        self.conn.execute(
            "INSERT INTO owners (id, name, is_local, created_at) VALUES (?1, 'Local owner', 1, ?2)",
            params![id, crate::now_ms()],
        )?;
        Ok(id)
    }
}

/// Applies every migration newer than the file's `user_version`, each in its own transaction.
/// Returns the resulting version.
pub fn migrate(conn: &mut Connection) -> Result<u32> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(DbError::SchemaTooNew { found: current, known: SCHEMA_VERSION });
    }
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(conn: &Connection) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    fn tables(conn: &Connection) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn migrations_apply_on_empty_file_and_twice_without_change() {
        let dir = std::env::temp_dir().join(format!("terminescu-db-{}", crate::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.sqlite");
        let first = {
            let db = Db::open(&path).unwrap();
            schema(db.conn())
        };
        let db = Db::open(&path).unwrap();
        assert_eq!(schema(db.conn()), first);
        let v: u32 = db.conn().pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn migrations_create_every_table_of_d18() {
        let db = Db::open_in_memory().unwrap();
        let expected = [
            "commands", "context_files", "context_items", "documents", "extraction_rules",
            "owners", "panes", "pins", "prompt_versions", "prompts", "sessions", "side_tabs",
            "status_rules", "statuses", "tabs", "windows",
        ];
        assert_eq!(tables(db.conn()), expected);
    }

    #[test]
    fn migrations_put_owner_id_on_every_table_except_owners() {
        let db = Db::open_in_memory().unwrap();
        for table in tables(db.conn()).into_iter().filter(|t| t != "owners") {
            let notnull: Option<bool> = db
                .conn()
                .query_row(
                    "SELECT \"notnull\" FROM pragma_table_info(?1) WHERE name = 'owner_id'",
                    [&table],
                    |r| r.get(0),
                )
                .optional()
                .unwrap();
            assert_eq!(notnull, Some(true), "{table} needs owner_id NOT NULL");
        }
    }

    #[test]
    fn migrations_refuse_a_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        assert!(matches!(migrate(&mut conn), Err(DbError::SchemaTooNew { .. })));
    }

    #[test]
    fn local_owner_is_created_once() {
        let db = Db::open_in_memory().unwrap();
        let a = db.local_owner().unwrap();
        let b = db.local_owner().unwrap();
        assert_eq!(a, b);
        let n: i64 = db.conn().query_row("SELECT count(*) FROM owners", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Db::open_in_memory().unwrap();
        let err = db.conn().execute(
            "INSERT INTO windows (id, owner_id, position, created_at) VALUES ('w', 'nobody', 0, 0)",
            [],
        );
        assert!(err.is_err());
    }
}
