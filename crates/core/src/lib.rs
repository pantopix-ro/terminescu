//! Terminescu core: the single source of truth for windows, side tabs, tabs and their
//! contexts (D6), persisted in SQLite (D18).

pub mod db;
pub mod state;

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
