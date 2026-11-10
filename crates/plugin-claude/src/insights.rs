//! What a Claude session shows on its tab (add-claude-insights D4, D5, D7): context used, the
//! model, the permission mode, compaction, subagents and the todo list. Pure: the plugin feeds
//! it transcript lines and hook events and turns the result into decorations.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use terminescu_plugin_api::{Decoration, Slot, Target, Tone};

/// A model's context window when Settings › Claude names none.
pub const DEFAULT_WINDOW: u64 = 200_000;
/// The meter turns amber here, in percent, unless Settings › Claude says otherwise.
pub const DEFAULT_THRESHOLD: u64 = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TodoState {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub content: String,
    pub state: TodoState,
    /// What the item reads as while in progress, as "Running the tests".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

/// One session's signals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Insight {
    /// Context used by the latest assistant entry: input, cache creation and cache read tokens.
    pub tokens: Option<u64>,
    /// The model, from the transcript or `SessionStart`.
    pub model: Option<String>,
    /// `permission_mode` of the latest hook event.
    pub mode: Option<String>,
    /// The latest `TodoWrite` list; `None` before the first one.
    pub todos: Option<Vec<Todo>>,
    pub compacting: bool,
    /// Ids of the subagents running now.
    pub subagents: BTreeSet<String>,
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

impl Insight {
    /// One transcript line. True when something shown changed. A subagent's entries
    /// (`isSidechain`) and lines of another shape are ignored.
    pub fn on_line(&mut self, line: &[u8]) -> bool {
        // Most lines are tool results and user text: skip the parse for those.
        if !contains(line, b"\"assistant\"") {
            return false;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            return false;
        };
        if str_of(&v, "type") != Some("assistant")
            || v.get("isSidechain") == Some(&Value::Bool(true))
        {
            return false;
        }
        let Some(message) = v.get("message") else {
            return false;
        };
        let before = self.clone();
        if let Some(model) = str_of(message, "model").filter(|m| !m.starts_with('<')) {
            self.model = Some(model.into());
        }
        if let Some(u) = message.get("usage") {
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            let used =
                n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
            if used > 0 {
                self.tokens = Some(used);
            }
        }
        for block in message
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if str_of(block, "type") == Some("tool_use")
                && str_of(block, "name") == Some("TodoWrite")
                && let Some(todos) = block.get("input").and_then(todos_of)
            {
                self.todos = Some(todos);
            }
        }
        *self != before
    }

    /// One hook event of the session. True when something shown changed.
    pub fn on_hook(&mut self, event: &Value) -> bool {
        let before = self.clone();
        if let Some(mode) = str_of(event, "permission_mode") {
            self.mode = Some(mode.into());
        }
        match str_of(event, "hook_event_name") {
            Some("SessionStart") => {
                if let Some(m) = str_of(event, "model").filter(|m| !m.is_empty()) {
                    self.model = Some(m.into());
                }
                self.compacting = false;
                if str_of(event, "source") != Some("compact") {
                    self.subagents.clear();
                }
            }
            Some("PreCompact") => self.compacting = true,
            Some("Stop") => self.compacting = false,
            Some("SubagentStart") => {
                if let Some(id) = str_of(event, "agent_id") {
                    self.subagents.insert(id.into());
                }
            }
            Some("SubagentStop") => {
                if let Some(id) = str_of(event, "agent_id") {
                    self.subagents.remove(id);
                }
            }
            _ => {}
        }
        *self != before
    }

    /// The decorations of the session's tab, in a stable order.
    pub fn decorations(&self, tab: &str, look: &Look) -> Vec<Decoration> {
        let target = Target::Tab(tab.into());
        let item = |slot, key: &str, text: String, tone, tip: Option<String>| Decoration {
            target: target.clone(),
            slot,
            key: key.into(),
            text,
            tone,
            tip,
        };
        let mut out = Vec::new();
        if let Some(used) = self.tokens {
            let window = look.window(self.model.as_deref());
            let pct = (used.saturating_mul(100) / window.max(1)).min(999);
            let tone = if pct >= look.threshold {
                Tone::Warn
            } else {
                Tone::Neutral
            };
            let model = self
                .model
                .as_deref()
                .map(|m| format!(" · {m}"))
                .unwrap_or_default();
            out.push(item(
                Slot::Meter,
                "context",
                format!("{pct}%"),
                tone,
                Some(format!(
                    "{} of {} tokens{model}",
                    short(used),
                    short(window)
                )),
            ));
        }
        if let Some(m) = &self.model {
            out.push(item(
                Slot::Chip,
                "model",
                model_label(m),
                Tone::Neutral,
                Some(m.clone()),
            ));
        }
        if let Some(mode) = self.mode.as_deref().and_then(mode_label) {
            out.push(item(
                Slot::Chip,
                "mode",
                mode.into(),
                Tone::Neutral,
                Some("Permission mode".into()),
            ));
        }
        if self.compacting {
            out.push(item(
                Slot::Subtitle,
                "compacting",
                "compacting".into(),
                Tone::Neutral,
                None,
            ));
        }
        match self.subagents.len() {
            0 => {}
            1 => out.push(item(
                Slot::Badge,
                "subagents",
                "1 subagent".into(),
                Tone::Neutral,
                None,
            )),
            n => out.push(item(
                Slot::Badge,
                "subagents",
                format!("{n} subagents"),
                Tone::Neutral,
                None,
            )),
        }
        out
    }
}

/// The meter's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    /// Percent at which the meter turns amber.
    pub threshold: u64,
    /// Context window by model id; a key also matches a model id it starts.
    pub windows: HashMap<String, u64>,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            windows: HashMap::new(),
        }
    }
}

impl Look {
    /// From the plugin's settings `meterThreshold` (percent) and `contextWindows`
    /// (`{model: tokens}`); a bad value falls back to the default.
    pub fn from_settings(threshold: Option<Value>, windows: Option<Value>) -> Self {
        let threshold = threshold
            .and_then(|v| v.as_u64())
            .filter(|t| (1..=100).contains(t))
            .unwrap_or(DEFAULT_THRESHOLD);
        let windows = windows
            .and_then(|v| v.as_object().cloned())
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| v.as_u64().filter(|n| *n > 0).map(|n| (k, n)))
                    .collect()
            })
            .unwrap_or_default();
        Self { threshold, windows }
    }

    pub fn window(&self, model: Option<&str>) -> u64 {
        let Some(model) = model else {
            return DEFAULT_WINDOW;
        };
        if let Some(n) = self.windows.get(model) {
            return *n;
        }
        // The longest key that starts the id, so `claude-opus-5` covers its dated builds.
        self.windows
            .iter()
            .filter(|(k, _)| model.starts_with(k.as_str()))
            .max_by_key(|(k, _)| k.len())
            .map_or(DEFAULT_WINDOW, |(_, n)| *n)
    }
}

/// A `TodoWrite` input as items, or `None` for any other shape (D5).
fn todos_of(input: &Value) -> Option<Vec<Todo>> {
    input
        .get("todos")?
        .as_array()?
        .iter()
        .map(|t| {
            let state = match str_of(t, "status")? {
                "pending" => TodoState::Pending,
                "in_progress" => TodoState::InProgress,
                "completed" => TodoState::Completed,
                _ => return None,
            };
            Some(Todo {
                content: str_of(t, "content")?.into(),
                state,
                active_form: str_of(t, "activeForm").map(Into::into),
            })
        })
        .collect()
}

/// `claude-opus-5-5` reads as `opus-5-5`; a dated build drops its date.
fn model_label(model: &str) -> String {
    let m = model.strip_prefix("claude-").unwrap_or(model);
    let m = match m.rsplit_once('-') {
        Some((head, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => m,
    };
    m.chars().take(24).collect()
}

/// The chip of a mode other than the default.
fn mode_label(mode: &str) -> Option<&'static str> {
    match mode {
        "plan" => Some("plan"),
        "acceptEdits" => Some("accept edits"),
        "bypassPermissions" => Some("bypass"),
        "dontAsk" => Some("don't ask"),
        _ => None,
    }
}

/// 172000 reads as `172k`, 1000000 as `1M`, 1500000 as `1.5M`.
fn short(n: u64) -> String {
    if n >= 1_000_000 {
        let tenths = (n + 50_000) / 100_000;
        if tenths.is_multiple_of(10) {
            format!("{}M", tenths / 10)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    } else if n >= 1000 {
        format!("{}k", (n + 500) / 1000)
    } else {
        n.to_string()
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant(usage: Value, content: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "type": "assistant",
            "isSidechain": false,
            "message": { "model": "claude-opus-5-5", "usage": usage, "content": content }
        }))
        .unwrap()
    }

    #[test]
    fn the_meter_reads_context_used_of_the_window() {
        let mut i = Insight::default();
        let line = assistant(
            json!({ "input_tokens": 2000, "cache_creation_input_tokens": 10000, "cache_read_input_tokens": 160000, "output_tokens": 900 }),
            json!([]),
        );
        assert!(i.on_line(&line));
        assert_eq!(i.tokens, Some(172_000));
        let d = i.decorations("t1", &Look::default());
        assert_eq!(d[0].text, "86%");
        assert_eq!(d[0].tone, Tone::Warn);
        assert_eq!(
            d[0].tip.as_deref(),
            Some("172k of 200k tokens · claude-opus-5-5")
        );
        let big = Look::from_settings(Some(json!(80)), Some(json!({ "claude-opus-5": 1_000_000 })));
        let d = i.decorations("t1", &big);
        assert_eq!((d[0].text.as_str(), d[0].tone), ("17%", Tone::Neutral));
        assert_eq!(
            d[0].tip.as_deref(),
            Some("172k of 1M tokens · claude-opus-5-5")
        );
    }

    #[test]
    fn a_subagent_line_and_odd_shapes_are_ignored() {
        let mut i = Insight::default();
        let side = serde_json::to_vec(&json!({
            "type": "assistant", "isSidechain": true,
            "message": { "usage": { "input_tokens": 5 } }
        }))
        .unwrap();
        assert!(!i.on_line(&side));
        assert!(!i.on_line(b"{\"type\":\"assistant\""));
        let odd = assistant(
            json!({}),
            json!([{ "type": "tool_use", "name": "TodoWrite", "input": { "todos": [{ "content": "x", "status": "maybe" }] } }]),
        );
        i.on_line(&odd);
        assert_eq!(i.todos, None);
    }

    #[test]
    fn the_latest_todo_list_wins() {
        let mut i = Insight::default();
        let todo = |status: &str| {
            assistant(
                json!({ "input_tokens": 10 }),
                json!([{ "type": "tool_use", "name": "TodoWrite", "input": { "todos": [
                    { "content": "Read retry.ts", "status": status, "activeForm": "Reading retry.ts" },
                    { "content": "Add backoff", "status": "pending", "activeForm": "Adding backoff" }
                ] } }]),
            )
        };
        i.on_line(&todo("in_progress"));
        i.on_line(&todo("completed"));
        let todos = i.todos.unwrap();
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].state, TodoState::Completed);
        assert_eq!(todos[1].active_form.as_deref(), Some("Adding backoff"));
    }

    #[test]
    fn hooks_set_mode_compaction_and_subagents() {
        let mut i = Insight::default();
        i.on_hook(&json!({ "hook_event_name": "SessionStart", "model": "claude-sonnet-5-5-20261001", "permission_mode": "default" }));
        i.on_hook(&json!({ "hook_event_name": "UserPromptSubmit", "permission_mode": "plan" }));
        i.on_hook(&json!({ "hook_event_name": "PreCompact" }));
        i.on_hook(&json!({ "hook_event_name": "SubagentStart", "agent_id": "a1" }));
        i.on_hook(&json!({ "hook_event_name": "SubagentStart", "agent_id": "a2" }));
        i.on_hook(&json!({ "hook_event_name": "SubagentStop", "agent_id": "a1" }));
        let d = i.decorations("t1", &Look::default());
        let texts: Vec<_> = d.iter().map(|d| (d.slot, d.text.as_str())).collect();
        assert_eq!(
            texts,
            [
                (Slot::Chip, "sonnet-5-5"),
                (Slot::Chip, "plan"),
                (Slot::Subtitle, "compacting"),
                (Slot::Badge, "1 subagent")
            ]
        );
        assert!(i.on_hook(&json!({ "hook_event_name": "SessionStart", "source": "compact" })));
        assert!(!i.compacting);
        assert_eq!(i.subagents.len(), 1, "compaction keeps the subagents");
        i.on_hook(&json!({ "hook_event_name": "UserPromptSubmit", "permission_mode": "default" }));
        assert!(
            i.decorations("t1", &Look::default())
                .iter()
                .all(|d| d.key != "mode")
        );
    }

    #[test]
    fn numbers_read_short() {
        assert_eq!(short(172_000), "172k");
        assert_eq!(short(1_000_000), "1M");
        assert_eq!(short(1_500_000), "1.5M");
        assert_eq!(short(512), "512");
    }
}
