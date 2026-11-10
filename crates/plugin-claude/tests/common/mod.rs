//! A recording `PluginCx` for the Claude plugin's tests (add-claude-insights).
#![allow(dead_code)]

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use terminescu_plugin_api::{Decoration, Id, PaneInfo, PluginCx, PluginError, Target};

#[derive(Default)]
pub struct Cx {
    pub panes: Mutex<Vec<PaneInfo>>,
    pub decorations: Mutex<HashMap<(Target, String, String), Decoration>>,
    pub settings: Mutex<HashMap<String, Value>>,
    pub row_data: Mutex<HashMap<(String, String), Value>>,
    pub emits: Mutex<Vec<(String, Value)>>,
    pub timers: Mutex<Vec<(Duration, String)>>,
    pub typed: Mutex<Vec<(String, String, String)>>,
    pub opened: Mutex<Vec<(String, String)>>,
    pub logs: Mutex<HashMap<String, PathBuf>>,
}

impl Cx {
    /// One pane `p1` in tab `t1` of row `s1`.
    pub fn one_pane(cwd: &str) -> Self {
        let cx = Self::default();
        cx.panes.lock().unwrap().push(PaneInfo {
            pane: "p1".into(),
            tab: "t1".into(),
            side_tab: "s1".into(),
            window: "w1".into(),
            cwd: Some(cwd.into()),
        });
        cx
    }

    /// The text of the decoration with `key` on `target`, any slot.
    pub fn text(&self, target: &Target, key: &str) -> Option<String> {
        self.decoration(target, key).map(|d| d.text)
    }

    pub fn decoration(&self, target: &Target, key: &str) -> Option<Decoration> {
        self.decorations
            .lock()
            .unwrap()
            .iter()
            .find(|((t, _, k), _)| t == target && k == key)
            .map(|(_, d)| d.clone())
    }
}

impl PluginCx for Cx {
    fn panes(&self) -> Vec<PaneInfo> {
        self.panes.lock().unwrap().clone()
    }
    fn workspace(&self) -> Value {
        Value::Null
    }
    fn side_tabs(&self, _: &str) -> Vec<Id> {
        self.panes().into_iter().map(|p| p.side_tab).collect()
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
        let key = (d.target.clone(), format!("{:?}", d.slot), d.key.clone());
        self.decorations.lock().unwrap().insert(key, d);
    }
    fn clear_decorations(&self, target: Option<&Target>) {
        let mut all = self.decorations.lock().unwrap();
        match target {
            Some(t) => all.retain(|k, _| &k.0 != t),
            None => all.clear(),
        }
    }
    fn open_text(&self, side_tab: &str, path: &str) -> Result<(), PluginError> {
        self.opened
            .lock()
            .unwrap()
            .push((side_tab.into(), path.into()));
        Ok(())
    }
    fn type_in_new_tab(&self, side_tab: &str, cwd: &str, line: &str) -> Result<(), PluginError> {
        self.typed
            .lock()
            .unwrap()
            .push((side_tab.into(), cwd.into(), line.into()));
        Ok(())
    }
    fn new_row(&self, _: &str, _: Option<&str>, _: &str, _: &str) -> Result<Id, PluginError> {
        Ok("s9".into())
    }
    fn setting(&self, key: &str) -> Option<Value> {
        self.settings.lock().unwrap().get(key).cloned()
    }
    fn set_setting(&self, key: &str, value: Option<Value>) -> Result<(), PluginError> {
        let mut s = self.settings.lock().unwrap();
        match value {
            Some(v) => s.insert(key.into(), v),
            None => s.remove(key),
        };
        Ok(())
    }
    fn row_data(&self, side_tab: &str, key: &str) -> Option<Value> {
        self.row_data
            .lock()
            .unwrap()
            .get(&(side_tab.into(), key.into()))
            .cloned()
    }
    fn set_row_data(
        &self,
        side_tab: &str,
        key: &str,
        value: Option<Value>,
    ) -> Result<(), PluginError> {
        let mut d = self.row_data.lock().unwrap();
        let k = (side_tab.to_string(), key.to_string());
        match value {
            Some(v) => d.insert(k, v),
            None => d.remove(&k),
        };
        Ok(())
    }
    fn set_fact(&self, _: &Target, _: &str, _: Option<Value>) {}
    fn set_variable(&self, _: &Target, _: &str, _: Option<String>) {}
    fn emit(&self, event: &str, payload: Value) {
        self.emits.lock().unwrap().push((event.into(), payload));
    }
    fn schedule(&self, after: Duration, token: &str) {
        self.timers.lock().unwrap().push((after, token.into()));
    }
    fn agent_log(&self, session: &str) -> Option<PathBuf> {
        self.logs.lock().unwrap().get(session).cloned()
    }
}
