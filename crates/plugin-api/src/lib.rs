//! Terminescu's plugin API (add-plugin-architecture). Plugins are crates built into the app
//! and registered in its composition root (D1). A plugin observes core events (Observer, D3),
//! answers calls from its UI half, and acts on the app only through [`PluginCx`] (Facade, D4).
//! The core never names a plugin.

mod agent;
pub mod atomic;
pub mod baseline;
mod decoration;
pub mod process;
mod tail;

pub use agent::{
    Agent, AgentAdapter, AgentEvent, AgentState, AgentUpdate, AgentView, is_session_id,
};
pub use decoration::{Decoration, Slot, Target, Tone};
pub use tail::LineTail;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub type Id = String;

/// Who a plugin is, and how long its handlers may run (D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Stable, lower case: `claude`, `git`. Keys its settings, data and decorations.
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub event_budget: Duration,
    pub call_budget: Duration,
}

impl Manifest {
    pub const fn new(id: &'static str, name: &'static str, description: &'static str) -> Self {
        Self {
            id,
            name,
            description,
            event_budget: Duration::from_millis(500),
            call_budget: Duration::from_secs(2),
        }
    }
}

/// What happened in the core, as a plugin hears it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CoreEvent {
    /// The app started, or the plugin was switched on: rebuild decorations.
    AppLaunched,
    PaneSpawned {
        pane: Id,
        side_tab: Id,
        cwd: Option<String>,
    },
    CwdMoved {
        pane: Id,
        cwd: String,
    },
    CommandStarted {
        pane: Id,
        command: String,
    },
    CommandEnded {
        pane: Id,
        command: String,
        exit: Option<i32>,
    },
    /// A hook event from a pane, as the agent adapters got it.
    Hook {
        pane: Id,
        event: Value,
    },
    PaneClosed {
        pane: Id,
    },
    TabClosed {
        tab: Id,
    },
    SideTabClosed {
        side_tab: Id,
    },
    SideTabSelected {
        window: Id,
        side_tab: Id,
    },
    /// A timer this plugin set with [`PluginCx::schedule`] came due. No other plugin hears it.
    Timer {
        token: String,
    },
}

/// A terminal pane and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneInfo {
    pub pane: Id,
    pub tab: Id,
    pub side_tab: Id,
    pub window: Id,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    #[error("{0} is not a method of this plugin")]
    UnknownMethod(String),
    #[error("bad arguments: {0}")]
    BadArgs(String),
    #[error("{0}")]
    Failed(String),
    #[error("the plugin took longer than its budget")]
    Timeout,
    #[error("the plugin is off or paused")]
    Off,
}

/// The one way a plugin acts on the app (Facade, D4). Every method is scoped to the calling
/// plugin: settings, row data, decorations, facts and variables are its own.
pub trait PluginCx: Send + Sync {
    /// The terminal panes, with their tabs, rows, windows and last cwds.
    fn panes(&self) -> Vec<PaneInfo>;
    /// The workspace as the UI sees it, for anything [`Self::panes`] does not answer.
    fn workspace(&self) -> Value;
    /// The rows of the window, in sidebar order.
    fn side_tabs(&self, window: &str) -> Vec<Id>;

    fn add_context_item(&self, side_tab: &str, text: &str) -> Result<(), PluginError>;
    fn add_touched_file(
        &self,
        side_tab: &str,
        origin_tab: Option<&str>,
        path: &str,
        created: bool,
    ) -> Result<(), PluginError>;

    /// Sets a decoration, replacing the one with the same target, slot and key.
    fn set_decoration(&self, decoration: Decoration);
    /// Clears this plugin's decorations on `target`, or all of them.
    fn clear_decorations(&self, target: Option<&Target>);

    /// Opens a file in a text tab of the row, beside its shown pane.
    fn open_text(&self, side_tab: &str, path: &str) -> Result<(), PluginError>;
    /// A new login-shell tab in the row, in `cwd`, with `line` typed and no Enter.
    fn type_in_new_tab(&self, side_tab: &str, cwd: &str, line: &str) -> Result<(), PluginError>;
    /// Shows an absolute path that exists in Finder, a file selected in its folder.
    fn reveal(&self, path: &str) -> Result<(), PluginError>;
    /// A new row below `after` (the end when absent), named `title`, whose first shell starts
    /// in `cwd`. Returns its id.
    fn new_row(
        &self,
        window: &str,
        after: Option<&str>,
        title: &str,
        cwd: &str,
    ) -> Result<Id, PluginError>;

    fn setting(&self, key: &str) -> Option<Value>;
    fn set_setting(&self, key: &str, value: Option<Value>) -> Result<(), PluginError>;
    /// Data kept with a row; it goes when the row closes.
    fn row_data(&self, side_tab: &str, key: &str) -> Option<Value>;
    fn set_row_data(
        &self,
        side_tab: &str,
        key: &str,
        value: Option<Value>,
    ) -> Result<(), PluginError>;

    /// A named fact dynamic status rules can test, on a row or a tab. `None` clears it.
    fn set_fact(&self, target: &Target, name: &str, value: Option<Value>);
    /// A read-only prompt variable for a row or pane; a saved variable of the name wins.
    fn set_variable(&self, target: &Target, name: &str, value: Option<String>);

    /// An event for this plugin's UI half.
    fn emit(&self, event: &str, payload: Value);

    /// Sends this plugin [`CoreEvent::Timer`] with `token` after `after`, on its own worker and
    /// under its event budget. Setting a token again moves its time (a debounce). The host
    /// keeps at most 256 timers per plugin and drops them when the plugin goes off or pauses.
    fn schedule(&self, _after: Duration, _token: &str) {}

    /// The log file of an agent session: the one its hooks named, else what its adapter finds.
    fn agent_log(&self, _session: &str) -> Option<std::path::PathBuf> {
        None
    }
}

/// An entry the app shows on the first-run screen and runs from Uninstall Helpers: the hook a
/// plugin puts in another tool's settings, for one.
pub trait Helper: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn installed(&self) -> bool;
    fn install(&self) -> Result<(), PluginError>;
    fn uninstall(&self) -> Result<(), PluginError>;
}

/// A built-in plugin (Observer, D3). The host runs each on its own worker, under its budgets.
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> Manifest;
    fn on_event(&self, event: &CoreEvent, cx: &dyn PluginCx);
    /// A call from the plugin's UI half: `method` and its JSON arguments.
    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError>;
    fn helpers(&self) -> Vec<Arc<dyn Helper>> {
        Vec::new()
    }
    fn agent_adapter(&self) -> Option<Arc<dyn AgentAdapter>> {
        None
    }
}

/// A plugin as Settings › Plugins lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// Stopped after 5 failures in a minute, until switched off and on.
    pub paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records what a plugin did through the facade.
    #[derive(Default)]
    struct FakeCx {
        decorations: Mutex<Vec<Decoration>>,
        facts: Mutex<Vec<(String, Option<Value>)>>,
    }

    impl PluginCx for FakeCx {
        fn panes(&self) -> Vec<PaneInfo> {
            vec![PaneInfo {
                pane: "p1".into(),
                tab: "t1".into(),
                side_tab: "s1".into(),
                window: "w1".into(),
                cwd: Some("/tmp/shop-api".into()),
            }]
        }
        fn workspace(&self) -> Value {
            Value::Null
        }
        fn side_tabs(&self, _: &str) -> Vec<Id> {
            vec!["s1".into()]
        }
        fn add_context_item(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn add_touched_file(
            &self,
            _: &str,
            _: Option<&str>,
            _: &str,
            _: bool,
        ) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_decoration(&self, d: Decoration) {
            self.decorations.lock().unwrap().push(d);
        }
        fn clear_decorations(&self, _: Option<&Target>) {
            self.decorations.lock().unwrap().clear();
        }
        fn open_text(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn type_in_new_tab(&self, _: &str, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn reveal(&self, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn new_row(&self, _: &str, _: Option<&str>, _: &str, _: &str) -> Result<Id, PluginError> {
            Ok("s2".into())
        }
        fn setting(&self, _: &str) -> Option<Value> {
            None
        }
        fn set_setting(&self, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn row_data(&self, _: &str, _: &str) -> Option<Value> {
            None
        }
        fn set_row_data(&self, _: &str, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_fact(&self, _: &Target, name: &str, value: Option<Value>) {
            self.facts.lock().unwrap().push((name.into(), value));
        }
        fn set_variable(&self, _: &Target, _: &str, _: Option<String>) {}
        fn emit(&self, _: &str, _: Value) {}
    }

    /// Chips each pane's folder name on its tab.
    struct Folders;

    impl Plugin for Folders {
        fn manifest(&self) -> Manifest {
            Manifest::new("folders", "Folders", "Shows each tab's folder")
        }
        fn on_event(&self, event: &CoreEvent, cx: &dyn PluginCx) {
            if *event != CoreEvent::AppLaunched {
                return;
            }
            for p in cx.panes() {
                let name = p.cwd.as_deref().and_then(|c| c.rsplit('/').next());
                cx.set_decoration(Decoration {
                    target: Target::Tab(p.tab.clone()),
                    slot: Slot::Chip,
                    key: "folder".into(),
                    text: name.unwrap_or("?").into(),
                    tone: Tone::Neutral,
                    tip: None,
                });
                cx.set_fact(&Target::Tab(p.tab), "inRepo", Some(Value::Bool(true)));
            }
        }
        fn call(&self, method: &str, _: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
            match method {
                "count" => Ok(Value::from(cx.panes().len())),
                other => Err(PluginError::UnknownMethod(other.into())),
            }
        }
    }

    /// An agent that runs on `start` and finishes on a `done` line.
    struct Echo;

    impl AgentAdapter for Echo {
        fn id(&self) -> &'static str {
            "echo"
        }
        fn accepts(&self, event: &Value) -> bool {
            event.get("echo").is_some()
        }
        fn on_hook(&self, _: Option<Agent>, event: &Value) -> Option<Agent> {
            Some(Agent {
                session: event["echo"].as_str()?.into(),
                transcript: None,
                state: AgentState::Running,
                last_message: None,
                reason: None,
            })
        }
        fn on_line(&self, agent: &mut Agent, line: &[u8]) -> bool {
            let done = line == b"done";
            if done {
                agent.state = AgentState::Done;
            }
            done
        }
        fn events(&self, _: &Value) -> Vec<AgentEvent> {
            Vec::new()
        }
        fn resume_command(&self, session: &str) -> Option<String> {
            is_session_id(session).then(|| format!("echo --resume {session}"))
        }
        fn find_transcript(&self, _: &str) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[test]
    fn a_fake_plugin_and_adapter_run() {
        let cx = FakeCx::default();
        let plugin: Arc<dyn Plugin> = Arc::new(Folders);
        assert_eq!(plugin.manifest().event_budget, Duration::from_millis(500));
        plugin.on_event(&CoreEvent::AppLaunched, &cx);
        let chips = cx.decorations.lock().unwrap().clone();
        assert_eq!(chips.len(), 1);
        assert_eq!(chips[0].text, "shop-api");
        assert_eq!(cx.facts.lock().unwrap().len(), 1);
        assert_eq!(plugin.call("count", Value::Null, &cx), Ok(Value::from(1)));
        assert!(matches!(
            plugin.call("nope", Value::Null, &cx),
            Err(PluginError::UnknownMethod(_))
        ));

        let echo: Arc<dyn AgentAdapter> = Arc::new(Echo);
        let event = serde_json::json!({ "echo": "s-1" });
        assert!(echo.accepts(&event));
        let mut agent = echo.on_hook(None, &event).unwrap();
        assert!(!echo.on_line(&mut agent, b"working"));
        assert!(echo.on_line(&mut agent, b"done"));
        assert_eq!(agent.view().state, AgentState::Done);
        assert_eq!(echo.resume_command("-x"), None);
    }

    #[test]
    fn core_events_serialize_for_the_ui() {
        let ev = CoreEvent::CommandEnded {
            pane: "p1".into(),
            command: "npm test".into(),
            exit: Some(1),
        };
        assert_eq!(
            serde_json::to_value(&ev).unwrap(),
            serde_json::json!({ "type": "commandEnded", "pane": "p1", "command": "npm test", "exit": 1 })
        );
    }
}
