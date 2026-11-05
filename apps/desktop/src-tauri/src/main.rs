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
use terminescu_core::agent::AgentUpdate;
use terminescu_core::attention::{self, Attention};
use terminescu_core::commands::{self, NewCommand, SavedCommand, Target};
use terminescu_core::context;
use terminescu_core::core_socket;
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::hook_install;
use terminescu_core::launcher;
use terminescu_core::persist;
use terminescu_core::prompts::{self, NewPrompt, Prompt};
use terminescu_core::runtime::Runtime;
use terminescu_core::settings;
use terminescu_core::state::{Command, CommandError, Core, Id, Snapshot, Workspace};
use terminescu_core::status::{Auto, Look, ManualStatus, RuleKind, Shown};
use terminescu_core::status_store::{self, SavedStatus, StatusConfig};
use terminescu_core::vars;
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
/// Saved commands changed; windows reload [`commands_list`].
const COMMANDS_EVENT: &str = "host://commands";
const SETTINGS_EVENT: &str = "host://settings";
/// To one window: focus this pane, after `⌘⇧A` picked it (workbench-ui "Attention signals").
const REVEAL_EVENT: &str = "host://reveal-pane";
/// To one window: play the chime, when Settings › Notifications turned it on.
const CHIME_EVENT: &str = "host://chime";
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
    attention: Arc<Mutex<Attention>>,
}

/// Focuses the pane that has waited longest, raising its window. `None` when nothing waits.
#[tauri::command]
fn attention_jump(app: AppHandle, state: State<'_, AppState>) -> Option<Id> {
    let ws = state.runtime.snapshot().workspace;
    let pane = {
        let mut a = lock(&state.attention);
        if a.retain(&ws) {
            set_badge(&app, a.count());
        }
        a.oldest().cloned()?
    };
    let place = attention::place_of(&ws, &pane)?;
    if let Some(w) = app.get_webview_window(&place.window) {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if let Err(e) = app.emit_to(place.window.as_str(), REVEAL_EVENT, &pane) {
        eprintln!("terminescu: jump to the waiting pane not sent: {e}");
    }
    Some(pane)
}

#[tauri::command]
fn attention_settings(state: State<'_, AppState>) -> Result<attention::Settings, String> {
    attention::settings(&lock(&state.db)).map_err(|e| e.to_string())
}

#[tauri::command]
fn attention_set(state: State<'_, AppState>, settings: attention::Settings) -> Result<(), String> {
    attention::set_settings(&lock(&state.db), settings).map_err(|e| e.to_string())
}

/// The line a launcher command types, filled from its side tab's variables. A launcher runs
/// with no one to ask, so a missing name refuses it.
fn launcher_bytes(db: &Db, rt: &Runtime, command: &SavedCommand) -> Result<Vec<u8>, String> {
    let side = command
        .side_tab_id
        .as_deref()
        .ok_or("a launcher command belongs to one side tab")?;
    let daemon = rt.daemon();
    let vars = vars::list(db, daemon.as_deref(), side).map_err(|e| e.to_string())?;
    let resolved = commands::resolve(&command.body, &vars, &BTreeMap::new());
    if !resolved.missing.is_empty() {
        return Err(format!(
            "{} needs a value for {} in its side tab first",
            command.name,
            resolved.missing.join(", ")
        ));
    }
    if !resolved.unsafe_for_run.is_empty() {
        return Err(format!(
            "{} holds a line break, so {} cannot run on its own",
            resolved.unsafe_for_run.join(", "),
            command.name
        ));
    }
    Ok(commands::type_bytes(&resolved.text, true))
}

/// Starts a launcher command: in its tab when that tab still exists, else in a new tab of its
/// side tab, which it then remembers. Takes the database lock only briefly: opening a tab saves
/// the layout, which locks the database too.
fn launch(db: &Mutex<Db>, rt: &Runtime, command: &SavedCommand) -> Result<(), String> {
    let bytes = launcher_bytes(&lock(db), rt, command)?;
    let ws = rt.workspace();
    let pane = command.tab_id.as_deref().and_then(|tab| {
        ws.windows
            .iter()
            .flat_map(|w| &w.side_tabs)
            .flat_map(|s| &s.tabs)
            .find(|t| t.id == tab)
            .and_then(|t| t.layout.terminals().into_iter().next().cloned())
    });
    if let Some(pane) = pane {
        rt.type_into(&pane, bytes);
        return Ok(());
    }
    let side = command.side_tab_id.clone().unwrap_or_default();
    let pane = rt
        .command_then_type(Command::CreateTab { side_tab_id: side }, bytes)
        .map_err(|e| e.to_string())?;
    let tab = pane.and_then(|p| rt.workspace().locate_pane(&p).map(|(_, t)| t.id.clone()));
    commands::set_tab(&lock(db), &command.id, tab.as_deref()).map_err(|e| e.to_string())
}

/// Hands the runtime every launcher side tab and its commands' tabs, for `n/m up`.
fn refresh_launchers(db: &Db, rt: &Runtime) {
    let list = match commands::launchers(db) {
        Ok(list) => list,
        Err(e) => return eprintln!("terminescu: launchers not loaded: {e}"),
    };
    let mut map: std::collections::HashMap<Id, Vec<Option<Id>>> = Default::default();
    for c in list {
        if let Some(side) = c.side_tab_id {
            map.entry(side).or_default().push(c.tab_id);
        }
    }
    rt.set_launchers(map);
}

/// After a reboot each launcher tab has a fresh shell: run its command there again (D23).
fn rerun_launchers(db: &Db, rt: &Runtime) {
    let fresh = rt.take_respawned();
    if fresh.is_empty() {
        return;
    }
    let ws = rt.workspace();
    for c in commands::launchers(db).unwrap_or_default() {
        let Some(tab) = c.tab_id.as_deref() else {
            continue;
        };
        let first = ws
            .windows
            .iter()
            .flat_map(|w| &w.side_tabs)
            .flat_map(|s| &s.tabs)
            .find(|t| t.id == tab)
            .and_then(|t| t.layout.terminals().into_iter().next().cloned());
        if let Some(pane) = first.filter(|p| fresh.contains(p)) {
            match launcher_bytes(db, rt, &c) {
                Ok(bytes) => {
                    rt.type_into(&pane, bytes);
                }
                Err(e) => eprintln!("terminescu: launcher not rerun: {e}"),
            }
        }
    }
}

/// Runs the launcher commands that are down again: one, or every one of a side tab.
#[tauri::command]
fn launcher_restart(
    state: State<'_, AppState>,
    side_tab: Id,
    id: Option<Id>,
) -> Result<(), String> {
    let rt = &state.runtime;
    let statuses = rt.statuses();
    let down = |c: &SavedCommand| {
        c.tab_id
            .as_ref()
            .and_then(|t| statuses.get(t))
            .is_none_or(|s| s.auto != Auto::Running)
    };
    let list: Vec<SavedCommand> = commands::launchers(&lock(&state.db))
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|c| c.side_tab_id.as_deref() == Some(side_tab.as_str()))
        .filter(|c| id.as_ref().is_none_or(|id| *id == c.id))
        .filter(down)
        .collect();
    let mut errors = Vec::new();
    for c in &list {
        if let Err(e) = launch(&state.db, rt, c) {
            errors.push(e);
        }
    }
    refresh_launchers(&lock(&state.db), rt);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[tauri::command]
fn commands_list(
    state: State<'_, AppState>,
    side_tab: Option<Id>,
) -> Result<Vec<SavedCommand>, String> {
    commands::list(&lock(&state.db), side_tab.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn commands_all(state: State<'_, AppState>) -> Result<Vec<SavedCommand>, String> {
    commands::all(&lock(&state.db)).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Result<settings::Settings, String> {
    settings::get(&lock(&state.db)).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_set(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: settings::Settings,
) -> Result<settings::Settings, String> {
    let saved = settings::set(&lock(&state.db), settings).map_err(|e| e.to_string())?;
    let _ = app.emit(SETTINGS_EVENT, ());
    Ok(saved)
}

#[tauri::command]
fn command_create(
    app: AppHandle,
    state: State<'_, AppState>,
    command: NewCommand,
) -> Result<SavedCommand, String> {
    let saved = commands::create(&lock(&state.db), &command).map_err(|e| e.to_string())?;
    let started = saved
        .launcher
        .then(|| launch(&state.db, &state.runtime, &saved));
    refresh_launchers(&lock(&state.db), &state.runtime);
    let _ = app.emit(COMMANDS_EVENT, ());
    match started {
        Some(Err(e)) => Err(format!("saved, but not started: {e}")),
        _ => commands::get(&lock(&state.db), &saved.id).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
fn command_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Id,
    command: NewCommand,
) -> Result<SavedCommand, String> {
    let (before, saved) = {
        let db = lock(&state.db);
        let before = commands::get(&db, &id).map_err(|e| e.to_string())?;
        let saved = commands::update(&db, &id, &command).map_err(|e| e.to_string())?;
        (before, saved)
    };
    let started =
        (saved.launcher && !before.launcher).then(|| launch(&state.db, &state.runtime, &saved));
    refresh_launchers(&lock(&state.db), &state.runtime);
    let _ = app.emit(COMMANDS_EVENT, ());
    match started {
        Some(Err(e)) => Err(format!("saved, but not started: {e}")),
        _ => commands::get(&lock(&state.db), &id).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
fn command_delete(app: AppHandle, state: State<'_, AppState>, id: Id) -> Result<(), String> {
    let db = lock(&state.db);
    commands::delete(&db, &id).map_err(|e| e.to_string())?;
    refresh_launchers(&db, &state.runtime);
    let _ = app.emit(COMMANDS_EVENT, ());
    Ok(())
}

/// How a run went: names to ask for, names whose value must not meet Enter, or the pane that
/// got the line.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum RunOutcome {
    Missing { names: Vec<String> },
    Unsafe { names: Vec<String> },
    Done { pane: Option<Id> },
}

/// Runs (`run`) or inserts a saved command from a side tab, filled from that side tab's
/// variables and `extra`. `target` overrides the saved one; `pane` is the focused pane.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn command_run(
    state: State<'_, AppState>,
    window_id: Id,
    side_tab: Id,
    id: Id,
    target: Option<Target>,
    pane: Option<Id>,
    run: bool,
    extra: BTreeMap<String, String>,
) -> Result<RunOutcome, String> {
    let (command, vars) = {
        let db = lock(&state.db);
        let command = commands::get(&db, &id).map_err(|e| e.to_string())?;
        let daemon = state.runtime.daemon();
        let vars = vars::list(&db, daemon.as_deref(), &side_tab).map_err(|e| e.to_string())?;
        (command, vars)
    };
    if command.side_tab_id.as_ref().is_some_and(|s| *s != side_tab) {
        return Err("this command belongs to another side tab".into());
    }
    let resolved = commands::resolve(&command.body, &vars, &extra);
    if !resolved.missing.is_empty() {
        return Ok(RunOutcome::Missing {
            names: resolved.missing,
        });
    }
    if run && !resolved.unsafe_for_run.is_empty() {
        return Ok(RunOutcome::Unsafe {
            names: resolved.unsafe_for_run,
        });
    }
    let bytes = commands::type_bytes(&resolved.text, run);
    let rt = &state.runtime;
    let pane = match target.unwrap_or(command.target) {
        Target::Focused => {
            let pane = pane.ok_or("focus a terminal pane first")?;
            let ws = rt.workspace();
            if ws.locate_pane(&pane).map(|(s, _)| &s.id) != Some(&side_tab) {
                return Err("the focused pane is in another side tab".into());
            }
            if !rt.type_into(&pane, bytes) {
                return Err("this pane has no shell right now".into());
            }
            Some(pane)
        }
        Target::NewTab => rt
            .command_then_type(
                Command::CreateTab {
                    side_tab_id: side_tab,
                },
                bytes,
            )
            .map_err(|e| e.to_string())?,
        Target::NewSideTab => rt
            .command_then_type(
                Command::CreateSideTab {
                    window_id,
                    index: None,
                },
                bytes,
            )
            .map_err(|e| e.to_string())?,
    };
    Ok(RunOutcome::Done { pane })
}

/// The dock badge carries the waiting count; zero clears it.
fn set_badge(app: &AppHandle, count: usize) {
    let badge = (count > 0).then_some(count as i64);
    if let Some(w) = app.webview_windows().values().next()
        && let Err(e) = w.set_badge_count(badge)
    {
        eprintln!("terminescu: dock badge not set: {e}");
    }
}

/// Feeds agent updates into [`Attention`] off the runtime's locks: the badge follows the count,
/// and a pane that starts waiting posts a notification while no window has focus, and the chime
/// when it is on. The notification names the side tab and tab and plays no sound.
fn watch_attention(
    app: AppHandle,
    runtime: Runtime,
    db: Arc<Mutex<Db>>,
    attention: Arc<Mutex<Attention>>,
    updates: std::sync::mpsc::Receiver<AgentUpdate>,
) {
    use tauri_plugin_notification::NotificationExt;
    std::thread::spawn(move || {
        for u in updates {
            let ws = runtime.snapshot().workspace;
            let (fresh, count) = {
                let mut a = lock(&attention);
                let fresh = a.update(&u, terminescu_core::now_ms());
                a.retain(&ws);
                (fresh, a.count())
            };
            set_badge(&app, count);
            if !fresh {
                continue;
            }
            let Some(place) = attention::place_of(&ws, &u.pane) else {
                continue;
            };
            let settings = attention::settings(&lock(&db)).unwrap_or_default();
            let windows = app.webview_windows();
            let focused = windows.values().any(|w| w.is_focused().unwrap_or(false));
            if settings.notify && !focused {
                let shown = app
                    .notification()
                    .builder()
                    .title(format!("{} › {}", place.side_name, place.tab_name))
                    .body("Claude is waiting for you")
                    .show();
                if let Err(e) = shown {
                    eprintln!("terminescu: notification not posted: {e}");
                }
            }
            if settings.chime {
                let _ = app.emit_to(place.window.as_str(), CHIME_EVENT, ());
            }
        }
    });
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

/// Saved variables from SQLite, session and secret ones from the daemon; a secret comes without
/// its value (D21).
#[tauri::command(async)]
fn vars_list(
    state: State<'_, AppState>,
    side_tab: String,
) -> Result<Vec<context::Variable>, String> {
    let daemon = state.runtime.daemon();
    vars::list(&lock(&state.db), daemon.as_deref(), &side_tab).map_err(|e| e.to_string())
}

/// Adds a variable from the panel or an extract. A name in use is refused unless `replace`.
#[tauri::command(async)]
fn var_add(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: String,
    variable: vars::NewVariable,
    replace: bool,
) -> Result<context::Variable, String> {
    let daemon = state.runtime.daemon();
    let v = vars::add(
        &lock(&state.db),
        daemon.as_deref(),
        &side_tab,
        &variable,
        replace,
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &side_tab);
    Ok(v)
}

#[tauri::command(async)]
fn var_change(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: String,
    name: String,
    change: vars::VariableChange,
) -> Result<context::Variable, String> {
    let daemon = state.runtime.daemon();
    let v = vars::change(
        &lock(&state.db),
        daemon.as_deref(),
        &side_tab,
        &name,
        &change,
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &side_tab);
    Ok(v)
}

#[tauri::command(async)]
fn var_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: String,
    name: String,
) -> Result<(), String> {
    let daemon = state.runtime.daemon();
    vars::remove(&lock(&state.db), daemon.as_deref(), &side_tab, &name)
        .map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &side_tab);
    Ok(())
}

/// Every window refreshes its library and insert palette on this (D14).
const PROMPTS_EVENT: &str = "host://prompts";

fn prompt_changed(app: &AppHandle, r: prompts::Result<Prompt>) -> Result<Prompt, String> {
    let p = r.map_err(|e| e.to_string())?;
    let _ = app.emit(PROMPTS_EVENT, &p.id);
    Ok(p)
}

#[tauri::command(async)]
fn prompts_search(
    state: State<'_, AppState>,
    query: String,
    archived: bool,
) -> Result<Vec<Prompt>, String> {
    prompts::search(&lock(&state.db), &query, archived).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn prompt_create(
    app: AppHandle,
    state: State<'_, AppState>,
    prompt: NewPrompt,
) -> Result<Prompt, String> {
    let r = prompts::create(&lock(&state.db), &prompt);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_save(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<Prompt, String> {
    let r = prompts::save(&lock(&state.db), &id, &body);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
    tags: Vec<String>,
) -> Result<Prompt, String> {
    let r = prompts::update(&lock(&state.db), &id, &name, &tags);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_versions(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<prompts::Version>, String> {
    prompts::versions(&lock(&state.db), &id).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn prompt_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: u32,
) -> Result<Prompt, String> {
    let r = prompts::restore(&lock(&state.db), &id, version);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_duplicate(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Prompt, String> {
    let r = prompts::duplicate(&lock(&state.db), &id);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<Prompt, String> {
    let r = prompts::set_archived(&lock(&state.db), &id, archived);
    prompt_changed(&app, r)
}

#[tauri::command(async)]
fn prompt_used(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    prompts::mark_used(&lock(&state.db), &id).map_err(|e| e.to_string())?;
    let _ = app.emit(PROMPTS_EVENT, &id);
    Ok(())
}

#[tauri::command(async)]
fn items_list(state: State<'_, AppState>, side_tab: String) -> Result<Vec<context::Item>, String> {
    context::items(&lock(&state.db), &side_tab).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn item_add(
    app: AppHandle,
    state: State<'_, AppState>,
    side_tab: String,
    kind: context::ItemKind,
    value: String,
    source: String,
    origin_tab_id: Option<String>,
) -> Result<context::Item, String> {
    let item = context::add_item(
        &lock(&state.db),
        &side_tab,
        kind,
        &value,
        &source,
        origin_tab_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit(CONTEXT_EVENT, &side_tab);
    Ok(item)
}

#[tauri::command(async)]
fn item_remove(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    if let Some(side) = context::remove_item(&lock(&state.db), &id).map_err(|e| e.to_string())? {
        let _ = app.emit(CONTEXT_EVENT, side);
    }
    Ok(())
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
#[allow(clippy::type_complexity)]
fn start_runtime(
    app: &AppHandle,
    db: Arc<Mutex<Db>>,
) -> Result<
    (Runtime, Vec<String>, std::sync::mpsc::Receiver<AgentUpdate>),
    Box<dyn std::error::Error>,
> {
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
    if let Err(e) = serve_core_socket(&run.core_socket, hooks.clone(), db.clone()) {
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
        let gone: Vec<Id> = lock(&sides).difference(&now).cloned().collect();
        *lock(&sides) = now;
        if let (false, Some(rt)) = (gone.is_empty(), rt.get()) {
            // Session and secret variables end with their side tab (D21).
            if let Some(d) = rt.daemon().filter(|d| d.side_vars) {
                for side in &gone {
                    if let Err(e) = d.side_vars_drop(side) {
                        eprintln!("terminescu: variables of a closed side tab not dropped: {e}");
                    }
                }
            }
            match status_store::rules(&db) {
                Ok(rules) => rt.set_status_rules(rules),
                Err(e) => eprintln!("terminescu: status rules not reloaded: {e}"),
            }
            // Their commands went with them too.
            refresh_launchers(&db, rt);
            let _ = handle.emit(STATUS_CONFIG_EVENT, ());
            let _ = handle.emit(COMMANDS_EVENT, ());
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
    let (to_attention, updates) = std::sync::mpsc::channel();
    for u in runtime.agents() {
        let _ = to_attention.send(u);
    }
    runtime.on_agent(Box::new(move |update| {
        if let Err(e) = handle.emit(AGENT_EVENT, update) {
            eprintln!("terminescu: agent broadcast failed: {e}");
        }
        let _ = to_attention.send(update.clone());
    }));
    let handle = app.clone();
    runtime.on_status(Box::new(move |map| {
        if let Err(e) = handle.emit(STATUS_EVENT, map) {
            eprintln!("terminescu: status broadcast failed: {e}");
        }
    }));
    let handle = app.clone();
    runtime.on_context(Box::new(move |side: &str| {
        let _ = handle.emit(CONTEXT_EVENT, side);
    }));
    runtime.set_status_rules(rules);
    let _ = hooks.set(runtime.clone());
    Ok((runtime, windows, updates))
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

/// Answers `terminescu hook`, `status` and saved `var set` (D34, D36).
fn serve_core_socket(
    path: &Path,
    runtime: Arc<OnceLock<Runtime>>,
    db: Arc<Mutex<Db>>,
) -> std::io::Result<()> {
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
            // `terminescu var set NAME value` without a mode: a saved variable, replacing any
            // value under that name except a secret's.
            CoreRequest::VarSet { name, value } => {
                let Some(rt) = runtime.get() else {
                    return CoreReply::error("starting", "Terminescu is still starting");
                };
                let Some((side, tab)) = rt.place_of_daemon(message.pane) else {
                    return CoreReply::error("no_side_tab", "this pane is not in the layout");
                };
                let new = vars::NewVariable {
                    name,
                    value,
                    mode: context::Mode::Saved,
                    ttl_secs: None,
                    source: "typed".into(),
                    origin_tab_id: Some(tab),
                };
                let daemon = rt.daemon();
                match vars::add(&lock(&db), daemon.as_deref(), &side, &new, true) {
                    Ok(_) => {
                        rt.context_changed(&side);
                        CoreReply::Ok
                    }
                    Err(e) => CoreReply::error("bad_var", e.to_string()),
                }
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
        .plugin(tauri_plugin_notification::init())
        .menu(build_menu)
        .on_menu_event(|app, event| send_action(app, event.id().as_ref()))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(dir.join("terminescu.sqlite"))?;
            db.local_owner()?;
            if let Err(e) = prompts::import_seeds(&db, prompts::SEEDS) {
                eprintln!("terminescu: seed prompts not imported: {e}");
            }
            let db = Arc::new(Mutex::new(db));
            let (runtime, windows, updates) = start_runtime(app.handle(), db.clone())?;
            let attention = Arc::new(Mutex::new(Attention::default()));
            watch_attention(
                app.handle().clone(),
                runtime.clone(),
                db.clone(),
                attention.clone(),
                updates,
            );
            {
                let db = lock(&db);
                refresh_launchers(&db, &runtime);
                rerun_launchers(&db, &runtime);
            }
            app.manage(AppState {
                runtime,
                db,
                attention,
            });
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
            var_add,
            var_change,
            var_remove,
            items_list,
            item_add,
            item_remove,
            prompts_search,
            prompt_create,
            prompt_save,
            prompt_update,
            prompt_versions,
            prompt_restore,
            prompt_duplicate,
            prompt_archive,
            prompt_used,
            attention_jump,
            commands_list,
            commands_all,
            settings_get,
            settings_set,
            command_create,
            command_update,
            command_delete,
            command_run,
            launcher_restart,
            attention_settings,
            attention_set,
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
