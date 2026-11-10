//! The Claude sessions a row's tabs ran (add-claude-insights D6), kept in the row's plugin data
//! under `sessions`: newest first, at most [`KEEP`]. The data goes with its row.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use terminescu_plugin_api::{PluginCx, is_session_id};

/// The row data key.
pub const KEY: &str = "sessions";
/// Sessions kept per row.
pub const KEEP: usize = 50;
/// Longest first-prompt line kept, in characters.
pub const PROMPT_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// The folder the session started in, absolute.
    pub cwd: String,
    /// Seconds since 1970.
    pub started: u64,
    #[serde(default)]
    pub ended: Option<u64>,
    /// The first prompt's first line.
    #[serde(default)]
    pub prompt: Option<String>,
}

/// The row's sessions, newest first; a bad value reads as none.
pub fn read(cx: &dyn PluginCx, row: &str) -> Vec<Session> {
    cx.row_data(row, KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn write(cx: &dyn PluginCx, row: &str, mut list: Vec<Session>) {
    list.sort_by_key(|s| std::cmp::Reverse(s.started));
    list.truncate(KEEP);
    if let Ok(v) = serde_json::to_value(&list) {
        let _ = cx.set_row_data(row, KEY, Some(v));
    }
}

fn first_line(prompt: &str) -> Option<String> {
    let line = prompt.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut out: String = line.chars().take(PROMPT_CHARS).collect();
    if line.chars().count() > PROMPT_CHARS {
        out.pop();
        out.push('…');
    }
    Some(out)
}

/// One hook event of `session` in a pane of `row`: `SessionStart` adds it, the first
/// `UserPromptSubmit` keeps its prompt's first line, `SessionEnd` its end.
pub fn on_hook(cx: &dyn PluginCx, row: &str, session: &str, event: &Value, now: u64) {
    if !is_session_id(session) {
        return;
    }
    let name = event.get("hook_event_name").and_then(Value::as_str);
    let mut list = read(cx, row);
    let at = list.iter().position(|s| s.id == session);
    match (name, at) {
        (Some("SessionStart"), None) => {
            let Some(cwd) = event
                .get("cwd")
                .and_then(Value::as_str)
                .filter(|c| Path::new(c).is_absolute())
            else {
                return;
            };
            list.push(Session {
                id: session.into(),
                cwd: cwd.into(),
                started: now,
                ended: None,
                prompt: None,
            });
        }
        (Some("UserPromptSubmit"), Some(i)) if list[i].prompt.is_none() => {
            let Some(p) = event
                .get("prompt")
                .and_then(Value::as_str)
                .and_then(first_line)
            else {
                return;
            };
            list[i].prompt = Some(p);
        }
        (Some("SessionEnd"), Some(i)) => list[i].ended = Some(now),
        _ => return,
    }
    write(cx, row, list);
}
