//! The plugins the app is built with, their facade and their Tauri commands
//! (add-plugin-architecture D1, D4, D6, D8). Only this file and `main.rs` name plugin crates.

use crate::{AppState, lock};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use terminescu_core::db::Db;
use terminescu_core::decorations::{Decorations, Shown};
use terminescu_core::plugin_host::{CxFactory, PluginHost};
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, Id};
use terminescu_core::{context, files, plugin_store};
use terminescu_plugin_api::{
    CoreEvent, Decoration, Helper, PaneInfo, Plugin, PluginCx, PluginError, PluginInfo, Target,
};

pub const DECORATIONS_EVENT: &str = "host://decorations";
/// An event from a plugin to its UI half: `{ plugin, event, payload }`.
pub const PLUGIN_EVENT: &str = "host://plugin";

#[derive(Default)]
pub struct Shared {
    pub decorations: Mutex<Decorations>,
}

pub struct Plugins {
    pub host: PluginHost,
    pub shared: Arc<Shared>,
    /// The enabled plugins' actions, composed by the UI, for the native menu (D7).
    pub menu: Mutex<Vec<MenuAction>>,
}

/// A plugin action as the menu shows it; `id` is `plugin.action`, `accel` empty when it has none.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MenuAction {
    pub id: String,
    pub label: String,
    pub menu: String,
    pub accel: String,
}

/// The plugin actions the menu shows now.
pub fn menu_items(app: &AppHandle) -> Vec<MenuAction> {
    app.try_state::<Arc<Plugins>>()
        .map(|p| lock(&p.menu).clone())
        .unwrap_or_default()
}

/// The plugins compiled in, in the order their decorators compose.
fn registered() -> Vec<Arc<dyn Plugin>> {
    let all: [Option<Arc<dyn Plugin>>; 2] = [
        #[cfg(feature = "plugin-claude")]
        Some(Arc::new(terminescu_plugin_claude::ClaudePlugin::new(
            crate::paths::bin_dir().join("terminescu"),
            &terminescu_protocol::paths::support_dir(),
        ))),
        #[cfg(not(feature = "plugin-claude"))]
        None,
        #[cfg(feature = "plugin-git")]
        Some(Arc::new(terminescu_plugin_git::GitPlugin::new())),
        #[cfg(not(feature = "plugin-git"))]
        None,
    ];
    all.into_iter().flatten().collect()
}

/// Every compiled-in plugin's helpers, on or off: the first run and Uninstall Helpers reach
/// them all.
pub fn helpers() -> Vec<Arc<dyn Helper>> {
    registered().iter().flat_map(|p| p.helpers()).collect()
}

/// A helper as the first-run screen and Settings › Shell show it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    pub installed: bool,
}

#[tauri::command]
pub fn helpers_list() -> Vec<HelperInfo> {
    helpers()
        .iter()
        .map(|h| HelperInfo {
            id: h.id().into(),
            label: h.label().into(),
            description: h.description().into(),
            installed: h.installed(),
        })
        .collect()
}

/// The runtime drives agents through the adapters of the plugins that are on (D2).
fn refresh_adapters(app: &AppHandle) {
    if let (Some(plugins), Some(state)) =
        (app.try_state::<Arc<Plugins>>(), app.try_state::<AppState>())
    {
        state.runtime.set_adapters(plugins.host.adapters());
    }
}

impl Plugins {
    pub fn start(app: &AppHandle, db: Arc<Mutex<Db>>) -> Self {
        let shared = Arc::new(Shared::default());
        let switches = plugin_store::switches(&lock(&db)).unwrap_or_else(|e| {
            eprintln!("terminescu: plugin switches not read: {e}");
            HashMap::new()
        });
        let cx: CxFactory = {
            let (app, db, shared) = (app.clone(), db.clone(), shared.clone());
            Arc::new(move |id| {
                Arc::new(Cx {
                    id,
                    app: app.clone(),
                    db: db.clone(),
                    shared: shared.clone(),
                }) as Arc<dyn PluginCx>
            })
        };
        let host = PluginHost::new(registered(), cx, &switches);
        // A plugin that stops, by switch or by pause, takes what it showed with it.
        let (handle, held) = (app.clone(), shared.clone());
        host.on_switch(Arc::new(move |id, live| {
            if !live {
                forget_plugin(&handle, &held, id);
            }
            refresh_adapters(&handle);
            // The Settings list shows a pause as soon as it happens.
            if let Some(plugins) = handle.try_state::<Arc<Plugins>>() {
                let _ = handle.emit("host://plugins", &plugins.host.list());
            }
        }));
        Self {
            host,
            shared,
            menu: Mutex::default(),
        }
    }

    pub fn dispatch(&self, event: &CoreEvent) {
        self.host.dispatch(event);
    }

    /// Drops what every plugin set on targets that left the workspace.
    pub fn forget(&self, app: &AppHandle, gone: &[Target]) {
        let mut changed = false;
        {
            let mut d = lock(&self.shared.decorations);
            for t in gone {
                changed |= d.forget(t);
            }
        }
        if changed {
            emit_decorations(app, &self.shared);
        }
    }
}

/// Core events reach the plugins; a closed pane, tab or row takes its decorations with it.
pub fn observe(app: &AppHandle, runtime: &Runtime, plugins: Arc<Plugins>) {
    runtime.set_adapters(plugins.host.adapters());
    // An adapter that panics counts against the plugin that provides it.
    let counted = plugins.clone();
    runtime.on_adapter_failure(Arc::new(move |adapter, what| {
        counted.host.adapter_failed(adapter, what);
    }));
    let app = app.clone();
    runtime.on_event(Box::new(move |event| {
        let gone = match &event {
            CoreEvent::PaneClosed { pane } => Some(Target::Pane(pane.clone())),
            CoreEvent::TabClosed { tab } => Some(Target::Tab(tab.clone())),
            CoreEvent::SideTabClosed { side_tab } => Some(Target::SideTab(side_tab.clone())),
            _ => None,
        };
        plugins.dispatch(&event);
        if let Some(t) = gone {
            plugins.forget(&app, &[t]);
        }
    }));
}

fn forget_plugin(app: &AppHandle, shared: &Shared, id: &str) {
    let changed = lock(&shared.decorations).clear(id, None);
    if let Some(state) = app.try_state::<AppState>() {
        state.runtime.forget_plugin(id);
    }
    if changed {
        emit_decorations(app, shared);
    }
}

fn emit_decorations(app: &AppHandle, shared: &Shared) {
    let list = lock(&shared.decorations).list();
    if let Err(e) = app.emit(DECORATIONS_EVENT, &list) {
        eprintln!("terminescu: decorations not sent: {e}");
    }
}

fn failed(e: impl std::fmt::Display) -> PluginError {
    PluginError::Failed(e.to_string())
}

/// What a plugin may do (Facade, D4), scoped to its id.
struct Cx {
    id: &'static str,
    app: AppHandle,
    db: Arc<Mutex<Db>>,
    shared: Arc<Shared>,
}

impl Cx {
    fn state(&self) -> Result<State<'_, AppState>, PluginError> {
        self.app
            .try_state::<AppState>()
            .ok_or_else(|| failed("Terminescu is still starting"))
    }
}

impl PluginCx for Cx {
    fn panes(&self) -> Vec<PaneInfo> {
        let Ok(state) = self.state() else {
            return Vec::new();
        };
        let ws = state.runtime.workspace();
        // One daemon list for every pane's cwd.
        let all: Vec<Id> = ws
            .windows
            .iter()
            .flat_map(|w| &w.side_tabs)
            .flat_map(|s| &s.tabs)
            .flat_map(|t| t.layout.terminals())
            .cloned()
            .collect();
        let mut cwds = state.runtime.live_cwds(&all);
        let mut out = Vec::new();
        for w in &ws.windows {
            for s in &w.side_tabs {
                for t in &s.tabs {
                    for p in t.layout.terminals() {
                        out.push(PaneInfo {
                            cwd: cwds.remove(p),
                            pane: p.clone(),
                            tab: t.id.clone(),
                            side_tab: s.id.clone(),
                            window: w.id.clone(),
                        });
                    }
                }
            }
        }
        out
    }

    fn workspace(&self) -> Value {
        self.state()
            .ok()
            .and_then(|s| serde_json::to_value(s.runtime.workspace()).ok())
            .unwrap_or(Value::Null)
    }

    fn side_tabs(&self, window: &str) -> Vec<Id> {
        let Ok(state) = self.state() else {
            return Vec::new();
        };
        let ws = state.runtime.workspace();
        ws.windows
            .iter()
            .find(|w| w.id == window)
            .map(|w| w.side_tabs.iter().map(|s| s.id.clone()).collect())
            .unwrap_or_default()
    }

    fn add_context_item(&self, side_tab: &str, text: &str) -> Result<(), PluginError> {
        let kind = if text.starts_with("https://") || text.starts_with("http://") {
            context::ItemKind::Link
        } else {
            context::ItemKind::Text
        };
        context::add_item(&lock(&self.db), side_tab, kind, text, self.id, None).map_err(failed)?;
        self.state()?.runtime.context_changed(side_tab);
        Ok(())
    }

    fn add_touched_file(
        &self,
        side_tab: &str,
        origin_tab: Option<&str>,
        path: &str,
        created: bool,
    ) -> Result<(), PluginError> {
        if !path.starts_with('/') {
            return Err(PluginError::BadArgs(format!("{path} is not absolute")));
        }
        let touch = files::Touch {
            path: path.into(),
            change: if created {
                files::Change::Added
            } else {
                files::Change::Modified
            },
            session: None,
            cwd: None,
        };
        files::record(&lock(&self.db), side_tab, origin_tab, &touch).map_err(failed)?;
        self.state()?.runtime.context_changed(side_tab);
        Ok(())
    }

    fn set_decoration(&self, decoration: Decoration) {
        if lock(&self.shared.decorations).set(self.id, decoration) {
            emit_decorations(&self.app, &self.shared);
        }
    }

    fn clear_decorations(&self, target: Option<&Target>) {
        if lock(&self.shared.decorations).clear(self.id, target) {
            emit_decorations(&self.app, &self.shared);
        }
    }

    /// The file shows once open, as a file `terminescu edit` opens does.
    fn open_text(&self, side_tab: &str, path: &str) -> Result<(), PluginError> {
        let state = self.state()?;
        let view = crate::docs::doc_open_file(state.clone(), side_tab.into(), path.into(), None)
            .map_err(failed)?;
        crate::docs::reveal(&self.app, &state.runtime, &view.id);
        Ok(())
    }

    fn type_in_new_tab(&self, side_tab: &str, cwd: &str, line: &str) -> Result<(), PluginError> {
        if line.contains(['\r', '\n']) {
            return Err(PluginError::BadArgs("the line holds a newline".into()));
        }
        let state = self.state()?;
        let cmd = Command::CreateTab {
            side_tab_id: side_tab.into(),
        };
        let (pane, _) = state
            .runtime
            .command_then_type_in(cmd, Some(cwd), line.as_bytes().to_vec())
            .map_err(failed)?;
        let pane = pane.ok_or_else(|| failed("the tab did not open"))?;
        // The new tab shows, so Enter there runs the line.
        let ws = state.runtime.workspace();
        if let Some(place) = crate::attention::place_of(&ws, &pane)
            && let Err(e) = self
                .app
                .emit_to(place.window.as_str(), crate::REVEAL_EVENT, &pane)
        {
            eprintln!("terminescu: the new tab not shown: {e}");
        }
        Ok(())
    }

    fn reveal(&self, path: &str) -> Result<(), PluginError> {
        let p = std::path::Path::new(path);
        if !p.is_absolute() || !p.exists() {
            return Err(PluginError::BadArgs(format!("no file {path}")));
        }
        // An absolute path never reads as an `open` option.
        std::process::Command::new("/usr/bin/open")
            .arg("-R")
            .arg(p)
            .spawn()
            .map(drop)
            .map_err(failed)
    }

    fn new_row(
        &self,
        window: &str,
        after: Option<&str>,
        title: &str,
        cwd: &str,
    ) -> Result<Id, PluginError> {
        let state = self.state()?;
        let ws = state.runtime.workspace();
        let w = ws
            .windows
            .iter()
            .find(|w| w.id == window)
            .ok_or_else(|| PluginError::BadArgs(format!("no window {window}")))?;
        let index = after
            .and_then(|a| w.side_tabs.iter().position(|s| s.id == a))
            .map(|i| i + 1);
        let before: Vec<Id> = w.side_tabs.iter().map(|s| s.id.clone()).collect();
        state
            .runtime
            .command_then_type_in(
                Command::CreateSideTab {
                    window_id: window.into(),
                    index,
                },
                Some(cwd),
                Vec::new(),
            )
            .map_err(failed)?;
        let id = self
            .side_tabs(window)
            .into_iter()
            .find(|s| !before.contains(s))
            .ok_or_else(|| failed("the row did not open"))?;
        state
            .runtime
            .command(Command::RenameSideTab {
                side_tab_id: id.clone(),
                title: Some(title.into()),
            })
            .map_err(failed)?;
        Ok(id)
    }

    fn setting(&self, key: &str) -> Option<Value> {
        plugin_store::setting(&lock(&self.db), self.id, key)
            .ok()
            .flatten()
    }

    fn set_setting(&self, key: &str, value: Option<Value>) -> Result<(), PluginError> {
        if key == plugin_store::ENABLED {
            return Err(PluginError::BadArgs("the switch belongs to the app".into()));
        }
        plugin_store::set_setting(&lock(&self.db), self.id, key, value.as_ref()).map_err(failed)
    }

    fn row_data(&self, side_tab: &str, key: &str) -> Option<Value> {
        plugin_store::row_data(&lock(&self.db), side_tab, self.id, key)
            .ok()
            .flatten()
    }

    fn set_row_data(
        &self,
        side_tab: &str,
        key: &str,
        value: Option<Value>,
    ) -> Result<(), PluginError> {
        plugin_store::set_row_data(&lock(&self.db), side_tab, self.id, key, value.as_ref())
            .map_err(failed)
    }

    /// Rules see it as `<plugin>.<name>`.
    fn set_fact(&self, target: &Target, name: &str, value: Option<Value>) {
        if !terminescu_core::provided::is_fact_name(&format!("{}.{name}", self.id)) {
            eprintln!("terminescu: plugin {} fact {name} has a bad name", self.id);
            return;
        }
        if let Ok(state) = self.state() {
            state.runtime.set_fact(self.id, target, name, value);
        }
    }

    fn set_variable(&self, target: &Target, name: &str, value: Option<String>) {
        if !context::is_variable_name(name) {
            eprintln!(
                "terminescu: plugin {} variable {name} has a bad name",
                self.id
            );
            return;
        }
        if let Ok(state) = self.state() {
            state
                .runtime
                .set_provided_variable(self.id, target, name, value);
        }
    }

    fn emit(&self, event: &str, payload: Value) {
        let body = serde_json::json!({ "plugin": self.id, "event": event, "payload": payload });
        if let Err(e) = self.app.emit(PLUGIN_EVENT, body) {
            eprintln!("terminescu: plugin event not sent: {e}");
        }
    }

    fn agent_log(&self, session: &str) -> Option<std::path::PathBuf> {
        self.state().ok()?.runtime.transcript_path(session)
    }
}

#[tauri::command(async)]
pub fn plugin_call(
    plugins: State<'_, Arc<Plugins>>,
    plugin: String,
    method: String,
    args: Value,
) -> Result<Value, String> {
    plugins
        .host
        .call(&plugin, &method, args)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn plugin_list(plugins: State<'_, Arc<Plugins>>) -> Vec<PluginInfo> {
    plugins.host.list()
}

#[tauri::command(async)]
pub fn plugin_set_enabled(
    app: AppHandle,
    plugins: State<'_, Arc<Plugins>>,
    state: State<'_, AppState>,
    plugin: String,
    enabled: bool,
) -> Result<Vec<PluginInfo>, String> {
    if !plugins.host.set_enabled(&plugin, enabled) {
        return Err(format!("no plugin {plugin}"));
    }
    plugin_store::set_enabled(&lock(&state.db), &plugin, enabled).map_err(|e| e.to_string())?;
    let list = plugins.host.list();
    let _ = app.emit("host://plugins", &list);
    Ok(list)
}

/// Puts the composed plugin actions in the native menu. An id must read `plugin.action`, so a
/// plugin item can never fire an app action.
#[tauri::command]
pub fn plugin_menu(
    app: AppHandle,
    plugins: State<'_, Arc<Plugins>>,
    state: State<'_, AppState>,
    items: Vec<MenuAction>,
) -> Result<(), String> {
    if let Some(bad) = items.iter().find(|i| !i.id.contains('.')) {
        return Err(format!("plugin action {} needs a plugin prefix", bad.id));
    }
    let same = {
        let mut menu = lock(&plugins.menu);
        let same = menu.len() == items.len()
            && menu.iter().zip(&items).all(|(a, b)| {
                (&a.id, &a.label, &a.menu, &a.accel) == (&b.id, &b.label, &b.menu, &b.accel)
            });
        *menu = items;
        same
    };
    if same {
        return Ok(());
    }
    // While Settings › Keys records a chord the menu stays chordless.
    let chords = lock(&state.recording).is_none();
    crate::set_chords(&app, &state.db, chords)
}

/// The UI selected a row: the core does not know which row a window shows (D3).
#[tauri::command]
pub fn plugin_row_selected(plugins: State<'_, Arc<Plugins>>, window: Id, side_tab: Id) {
    plugins.dispatch(&CoreEvent::SideTabSelected { window, side_tab });
}

#[tauri::command]
pub fn decorations(plugins: State<'_, Arc<Plugins>>) -> Vec<Shown> {
    lock(&plugins.shared.decorations).list()
}
