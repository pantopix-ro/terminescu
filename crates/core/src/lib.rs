//! Terminescu core: the single source of truth for windows, side tabs, tabs and their
//! contexts (D6), persisted in SQLite (D18).

pub mod agent;
mod atomic;
pub mod attention;
pub mod closed;
pub mod commands;
pub mod context;
pub mod core_socket;
pub mod daemon;
pub mod db;
pub mod documents;
pub mod extract;
pub mod feedback;
pub mod files;
pub mod hook_install;
pub mod keys;
pub mod labels;
pub mod launcher;
pub mod layout;
pub mod merge;
pub mod persist;
pub mod plugin_host;
pub mod plugin_store;
pub mod prompts;
pub mod runtime;
pub mod settings;
pub mod state;
pub mod status;
pub mod status_store;
pub mod term_import;
pub mod transcript;
pub mod uninstall;
pub mod vars;
pub mod views;

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
