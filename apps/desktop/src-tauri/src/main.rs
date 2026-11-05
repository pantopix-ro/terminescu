//! The Terminescu app: hosts the core and one webview per window (D6).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry};
use terminescu_core::context;
use terminescu_core::core_socket;
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::hook_install;
use terminescu_core::launcher;
use terminescu_core::persist;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, CommandError, Core, Id, Snapshot, Workspace};
use terminescu_core::status::{Auto, Look, ManualStatus, RuleKind, Shown};
use terminescu_core::status_store::{self, SavedStatus, StatusConfig};
use terminescu_core::views::ViewId;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest};
use terminescu_protocol::paths;

/// Event that carries every patch batch to every window.
const PATCHES_EVENT: &str = "host://patches";
const AGENT_EVENT: &str = "host://agent";
/// A side tab's pins or variables changed; the payload is its id.
const CONTEXT_EVENT: &str = "host://context";
/// Every tab's and side tab's shown status, whenever one changes (D15).
const STATUS_EVENT: &str = "host://status";
/// Saved statuses, rules or built-in looks changed; windows reload [`status_config`].
const STATUS_CONFIG_EVENT: &str = "host://status-config";
/// Event that carries an app action from the menu to the focused window (D17).
const ACTION_EVENT: &str = "menu://action";

/// The actions and their chords; the browser fallback reads the same file (D17).
const ACTIONS_JSON: &str = include_str!("../../../../packages/host/src/actions.json");

#[derive(Deserialize)]
struct ActionsFile {
    actions: Vec<ActionDef>,
}

#[derive(Deserialize)]
struct ActionDef {
    id: String,
    label: String,
    menu: String,
    accel: String,
}

struct AppState {
    runtime: Runtime,
    db: Arc<Mutex<Db>>,
}

#[tauri::command]
fn host_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.runtime.snapshot()
}

#[tauri::command]
fn host_agents(state: State<'_, AppState>) -> Vec<terminescu_core::agent::AgentUpdate> {
    state.runtime.agents()
}

#[derive(serde::Serialize)]
struct TranscriptChunk {
    entries: Vec<terminescu_core::transcript::Entry>,
    /// Where the next read starts.
    offset: u64,
    /// The file shrank or was replaced: `entries` start over from the top.
    reset: bool,
}

/// The entries added to a session's transcript since `offset` (task 4.4).
// File and database reads run on a worker thread, so a big transcript or a save in progress
// never stalls the window.
#[tauri::command(async)]
fn transcript_read(
    state: State<'_, AppState>,
    session: String,
    offset: u64,
) -> Result<TranscriptChunk, String> {
    let path = state
        .runtime
        .transcript_path(&session)
        .ok_or_else(|| format!("no transcript for session {session}"))?;
    let mut tail = terminescu_core::transcript::Tail::at(path, offset);
    let entries = tail.read().map_err(|e| e.to_string())?;
    let reset = tail.offset() < offset || entries.first().is_some_and(|e| e.offset < offset);
    Ok(TranscriptChunk {
        entries,
        offset: tail.offset(),
        reset,
    })
}

#[tauri::command(async)]
fn pins_list(state: State<'_, AppState>, side_tab: String) -> Result<Vec<context::Pin>, String> {
    context::pins(&lock(&state.db), &side_tab).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn pin_add(
    app: AppHandle,
    state: State<'_, AppState>,
    pin: context::Pin,
) -> Result<context::Pin, String> {
    let pin = context::add_pin(&lock(&state.db), &pin).map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &pin.side_tab_id);
    Ok(pin)
}

#[tauri::command(async)]
fn pin_remove(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    if let Some(side) = context::remove_pin(&lock(&state.db), &id).map_err(|e| e.to_string())? {
        let _ = app.emit(CONTEXT_EVENT, side);
    }
    Ok(())
}

#[tauri::command(async)]
fn vars_list(
    state: State<'_, AppState>,
    side_tab: String,
) -> Result<Vec<context::Variable>, String> {
    context::variables(&lock(&state.db), &side_tab).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn var_extract(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: String,
    name: String,
    value: String,
) -> Result<context::Variable, String> {
    let v = context::add_variable(&lock(&state.db), &side_tab, &name, &value, "extracted")
        .map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &side_tab);
    Ok(v)
}

#[tauri::command]
fn host_statuses(state: State<'_, AppState>) -> BTreeMap<Id, Shown> {
    state.runtime.statuses()
}

#[tauri::command(async)]
fn status_config(state: State<'_, AppState>) -> Result<StatusConfig, String> {
    status_store::config(&lock(&state.db)).map_err(|e| e.to_string())
}

/// Runs a change to the saved statuses, then hands the rules to the runtime and tells every
/// window to reload the config.
fn config_change<T>(
    app: &AppHandle,
    state: &AppState,
    f: impl FnOnce(&Db) -> Result<T, status_store::StoreError>,
) -> Result<T, String> {
    let db = lock(&state.db);
    let out = f(&db).map_err(|e| e.to_string())?;
    match status_store::rules(&db) {
        Ok(rules) => state.runtime.set_status_rules(rules),
        Err(e) => eprintln!("terminescu: status rules not reloaded: {e}"),
    }
    drop(db);
    let _ = app.emit(STATUS_CONFIG_EVENT, ());
    Ok(out)
}

#[tauri::command(async)]
fn status_save(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: Option<String>,
    look: Look,
) -> Result<SavedStatus, String> {
    config_change(&app, &state, |db| {
        status_store::add_status(db, side_tab.as_deref(), &look)
    })
}

/// Edits a saved status; every tab using it takes the new look.
#[tauri::command(async)]
fn status_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    look: Look,
) -> Result<SavedStatus, String> {
    let saved = config_change(&app, &state, |db| {
        status_store::update_status(db, &id, &look)
    })?;
    state
        .runtime
        .command(Command::RestyleStatus {
            status_id: id,
            status: Some(ManualStatus {
                status_id: Some(saved.id.clone()),
                look: saved.look.clone(),
            }),
        })
        .map_err(|e| e.to_string())?;
    Ok(saved)
}

/// Deletes a saved status and its rules; tabs using it go back to auto.
#[tauri::command(async)]
fn status_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    config_change(&app, &state, |db| status_store::delete_status(db, &id))?;
    state
        .runtime
        .command(Command::RestyleStatus {
            status_id: id,
            status: None,
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn status_make_global(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<SavedStatus, String> {
    config_change(&app, &state, |db| status_store::make_global(db, &id))
}

#[tauri::command(async)]
fn status_reorder(app: AppHandle, state: State<'_, AppState>, ids: Vec<Id>) -> Result<(), String> {
    config_change(&app, &state, |db| status_store::reorder(db, &ids))
}

#[tauri::command(async)]
fn rule_add(
    app: AppHandle,
    state: State<'_, AppState>,
    status_id: String,
    kind: RuleKind,
) -> Result<terminescu_core::status::Rule, String> {
    config_change(&app, &state, |db| {
        status_store::add_rule(db, &status_id, &kind)
    })
}

#[tauri::command(async)]
fn rule_toggle(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    config_change(&app, &state, |db| {
        status_store::set_rule_enabled(db, &id, enabled)
    })
}

#[tauri::command(async)]
fn rule_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    config_change(&app, &state, |db| status_store::delete_rule(db, &id))
}

/// Renames or recolors a built-in status; `None` restores its default look.
#[tauri::command(async)]
fn builtin_set(
    app: AppHandle,
    state: State<'_, AppState>,
    auto: Auto,
    look: Option<Look>,
) -> Result<(), String> {
    config_change(&app, &state, |db| {
        status_store::set_builtin(db, auto, look.as_ref())
    })
}

#[tauri::command]
fn host_command(state: State<'_, AppState>, command: Command) -> Result<u64, CommandError> {
    // The runtime emits each batch under its core lock, so windows see them in rev order.
    state.runtime.command(command)
}

/// Opens a view of a pane. Output goes to `on_output` as raw bytes (D33).
#[tauri::command]
fn pane_open(
    state: State<'_, AppState>,
    pane: String,
    cols: u16,
    rows: u16,
    on_output: Channel<InvokeResponseBody>,
) -> Result<ViewId, CommandError> {
    let sink =
        Arc::new(move |bytes: Vec<u8>| on_output.send(InvokeResponseBody::Raw(bytes)).is_ok());
    state.runtime.open_view(&pane, cols, rows, sink)
}

#[tauri::command]
fn pane_close(state: State<'_, AppState>, view: ViewId) {
    state.runtime.close_view(view);
}

#[tauri::command]
fn pane_ack(state: State<'_, AppState>, view: ViewId, bytes: u64) {
    state.runtime.view_ack(view, bytes);
}

#[tauri::command]
fn pane_resize(state: State<'_, AppState>, view: ViewId, cols: u16, rows: u16) {
    state.runtime.view_resize(view, cols, rows);
}

/// Keys and pastes. xterm.js binary data carries one byte per character.
#[tauri::command]
fn pane_input(state: State<'_, AppState>, view: ViewId, data: String, binary: bool) {
    let bytes = if binary {
        data.chars().map(|c| c as u32 as u8).collect()
    } else {
        data.into_bytes()
    };
    state.runtime.view_input(view, bytes);
}

/// Opens a link from a pane in the default browser. Only http and https leave the app, so the
/// url can never read as an `open` option.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(format!("not an http(s) link: {url}"));
    }
    std::process::Command::new("/usr/bin/open")
        .arg(&url)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// The sidecars sit next to the app binary: in `Contents/MacOS` when bundled, in
/// `target/debug` under `tauri dev`.
fn sidecar(name: &str) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent().unwrap_or(Path::new(".")).join(name))
}

/// Installs the daemon and CLI copies (D32, D36) and starts the runtime on the daemon, from the
/// saved layout when there is one (task 3.3). Returns the runtime and its window ids.
fn start_runtime(
    app: &AppHandle,
    db: Arc<Mutex<Db>>,
) -> Result<(Runtime, Vec<String>), Box<dyn std::error::Error>> {
    let bin = paths::bin_dir();
    let daemon = launcher::install(&sidecar("terminescu-ptyd")?, &bin)?;
    match launcher::install_cli(&sidecar("terminescu")?, &bin) {
        // Claude Code runs the hook from the copy, whose path survives updates (D36).
        Ok(cli) => match hook_install::install(&hook_install::user_settings(), &cli) {
            Ok(true) => eprintln!("terminescu: Claude Code hooks installed"),
            Ok(false) => {}
            Err(e) => eprintln!("terminescu: Claude Code hooks not installed: {e}"),
        },
        Err(e) => eprintln!("terminescu: CLI copy failed: {e}"),
    }
    let run = paths::run_paths();
    paths::ensure_private_dir(&run.dir)?;
    let hooks: Arc<OnceLock<Runtime>> = Arc::default();
    if let Err(e) = serve_core_socket(&run.core_socket, hooks.clone()) {
        eprintln!("terminescu: core socket not served, hooks and statuses stay off: {e}");
    }
    let log = paths::support_dir().join("logs/ptyd.log");
    let handle = app.clone();
    let emit = Box::new(move |batch: &terminescu_core::state::PatchBatch| {
        if let Err(e) = handle.emit(PATCHES_EVENT, batch) {
            eprintln!("terminescu: patch broadcast failed: {e}");
        }
    });
    let connect = Box::new(move |events| {
        launcher::ensure_running(
            &daemon,
            &run.ptyd_socket,
            &log,
            &[],
            Duration::from_secs(10),
        )?;
        DaemonClient::connect(&run.ptyd_socket, events)
    });
    let (saved, rules) = {
        let db = lock(&db);
        (persist::load(&db)?, status_store::rules(&db)?)
    };
    // Closing a side tab deletes its local statuses and their rules (ON DELETE CASCADE), so the
    // runtime gets the rules again and windows reload the config.
    let sides = Mutex::new(
        saved
            .as_ref()
            .map(|s| side_ids(&s.workspace))
            .unwrap_or_default(),
    );
    let rt = hooks.clone();
    let handle = app.clone();
    let persist = Box::new(move |saved: &persist::Saved| {
        let db = lock(&db);
        if let Err(e) = persist::save(&db, saved) {
            eprintln!("terminescu: layout not saved: {e}");
            return;
        }
        let now = side_ids(&saved.workspace);
        let closed = lock(&sides).difference(&now).next().is_some();
        *lock(&sides) = now;
        if let (true, Some(rt)) = (closed, rt.get()) {
            match status_store::rules(&db) {
                Ok(rules) => rt.set_status_rules(rules),
                Err(e) => eprintln!("terminescu: status rules not reloaded: {e}"),
            }
            let _ = handle.emit(STATUS_CONFIG_EVENT, ());
        }
    });
    let (runtime, windows) = Runtime::start(
        Core::new(Workspace::default()),
        saved,
        emit,
        connect,
        persist,
    )?;
    let handle = app.clone();
    runtime.on_agent(Box::new(move |update| {
        if let Err(e) = handle.emit(AGENT_EVENT, update) {
            eprintln!("terminescu: agent broadcast failed: {e}");
        }
    }));
    let handle = app.clone();
    runtime.on_status(Box::new(move |map| {
        if let Err(e) = handle.emit(STATUS_EVENT, map) {
            eprintln!("terminescu: status broadcast failed: {e}");
        }
    }));
    runtime.set_status_rules(rules);
    let _ = hooks.set(runtime.clone());
    Ok((runtime, windows))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn side_ids(ws: &Workspace) -> std::collections::HashSet<Id> {
    ws.windows
        .iter()
        .flat_map(|w| w.side_tabs.iter().map(|s| s.id.clone()))
        .collect()
}

/// Answers `terminescu hook`, `status` and saved `var set` (D34, D36). Saved variables arrive
/// with task 6.1; until then `var set` gets `not_built`, which the CLI turns into a warning.
fn serve_core_socket(path: &Path, runtime: Arc<OnceLock<Runtime>>) -> std::io::Result<()> {
    core_socket::serve(
        path,
        Arc::new(move |message: CoreMessage| match message.request {
            // The session a hook names drives the tab's agent state (task 4.2) and the resume
            // offer (task 3.4).
            CoreRequest::Hook { event } => {
                if let Some(rt) = runtime.get() {
                    rt.hook(message.pane, &event);
                }
                CoreReply::Ok
            }
            // A status from `terminescu status`; it lives until it expires or the app quits.
            CoreRequest::Status {
                label,
                color,
                expiry,
            } => match runtime.get() {
                Some(rt) => match rt.status_from_cli(message.pane, label, color, expiry) {
                    Ok(()) => CoreReply::Ok,
                    Err(e) => CoreReply::error("bad_status", e),
                },
                None => CoreReply::error("starting", "Terminescu is still starting"),
            },
            CoreRequest::VarSet { .. } => {
                CoreReply::error("not_built", "saved variables arrive in a later version")
            }
        }),
    )
}

fn open_window(app: &AppHandle, window_id: &str) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, window_id, WebviewUrl::App("index.html".into()))
        .title("Terminescu")
        .inner_size(1200.0, 760.0)
        .min_inner_size(640.0, 400.0);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    builder.build()?;
    Ok(())
}

/// The app menu (D17): the standard macOS menus plus every action from actions.json under
/// its menu, with its chord.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let file: ActionsFile =
        serde_json::from_str(ACTIONS_JSON).expect("actions.json is checked by the host tests");
    let mut sections: Vec<(String, Vec<MenuItem<Wry>>)> = Vec::new();
    for a in &file.actions {
        let item = MenuItem::with_id(app, &a.id, &a.label, true, Some(&a.accel))?;
        match sections.iter_mut().find(|(name, _)| *name == a.menu) {
            Some((_, items)) => items.push(item),
            None => sections.push((a.menu.clone(), vec![item])),
        }
    }
    let menu = Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                "Terminescu",
                true,
                &[
                    &PredefinedMenuItem::about(app, None, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::show_all(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
        ],
    )?;
    for (name, items) in &sections {
        let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items
            .iter()
            .map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect();
        let sub = Submenu::with_items(app, name, true, &refs)?;
        if name == "Window" {
            sub.prepend_items(&[
                &PredefinedMenuItem::minimize(app, None)?,
                &PredefinedMenuItem::fullscreen(app, None)?,
                &PredefinedMenuItem::separator(app)?,
            ])?;
            sub.set_as_windows_menu_for_nsapp()?;
        }
        menu.append(&sub)?;
    }
    Ok(menu)
}

/// Sends a menu action to the focused window only; other windows keep their state.
fn send_action(app: &AppHandle, id: &str) {
    let focused = app
        .webview_windows()
        .into_iter()
        .find(|(_, w)| w.is_focused().unwrap_or(false));
    if let Some((label, _)) = focused
        && let Err(e) = app.emit_to(label.as_str(), ACTION_EVENT, id)
    {
        eprintln!("terminescu: menu action not sent: {e}");
    }
}

fn main() {
    let app = tauri::Builder::default()
        .menu(build_menu)
        .on_menu_event(|app, event| send_action(app, event.id().as_ref()))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(dir.join("terminescu.sqlite"))?;
            db.local_owner()?;
            let db = Arc::new(Mutex::new(db));
            let (runtime, windows) = start_runtime(app.handle(), db.clone())?;
            app.manage(AppState { runtime, db });
            for id in &windows {
                open_window(app.handle(), id)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            host_snapshot,
            host_agents,
            transcript_read,
            pins_list,
            pin_add,
            pin_remove,
            vars_list,
            var_extract,
            host_statuses,
            status_config,
            status_save,
            status_update,
            status_delete,
            status_make_global,
            status_reorder,
            rule_add,
            rule_toggle,
            rule_delete,
            builtin_set,
            host_command,
            pane_open,
            pane_close,
            pane_ack,
            pane_resize,
            pane_input,
            open_url
        ])
        .build(tauri::generate_context!())
        .expect("Terminescu failed to start");
    app.run(|app, event| {
        // A last save with fresh cwds, so a reboot after quitting restarts shells where they were.
        if let RunEvent::Exit = event
            && let Some(state) = app.try_state::<AppState>()
        {
            state.runtime.stop();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_unique_id_a_known_menu_and_a_chord() {
        let file: ActionsFile = serde_json::from_str(ACTIONS_JSON).unwrap();
        let mut ids = std::collections::HashSet::new();
        for a in &file.actions {
            assert!(ids.insert(a.id.as_str()), "{} is listed twice", a.id);
            assert!(
                ["Shell", "View", "Window"].contains(&a.menu.as_str()),
                "{}",
                a.menu
            );
            assert!(a.accel.contains("Cmd+"), "{} needs a ⌘ chord (D17)", a.id);
            assert!(!a.label.is_empty());
        }
    }
}
