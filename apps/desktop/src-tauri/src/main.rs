//! The Terminescu app: hosts the core and one webview per window (D6).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod crashlog;
mod docs;
mod drag;
mod edit;
mod plugins;
mod public;
mod reopen;
mod updates;

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry};
use terminescu_core::attention::{self, Attention};
use terminescu_core::commands::{self, NewCommand, SavedCommand, Target};
use terminescu_core::context;
use terminescu_core::core_socket;
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::{Db, DbAccess};
use terminescu_core::keys;
use terminescu_core::labels::PaneLabel;
use terminescu_core::launcher;
use terminescu_core::merge;
use terminescu_core::persist;
use terminescu_core::prompts::{self, NewPrompt, Prompt};
use terminescu_core::runtime::Runtime;
use terminescu_core::settings;
use terminescu_core::state::{
    Command, CommandError, Core, Id, Patch, SideTab, Snapshot, Workspace,
};
use terminescu_core::status::{Auto, Look, ManualStatus, RuleKind, Shown};
use terminescu_core::status_store::{self, SavedStatus, StatusConfig};
use terminescu_core::term_import;
use terminescu_core::vars;
use terminescu_core::views::ViewId;
use terminescu_plugin_api::AgentUpdate;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest};
use terminescu_protocol::paths;

/// Event that carries every patch batch to every window.
const PATCHES_EVENT: &str = "host://patches";
const AGENT_EVENT: &str = "host://agent";
/// A side tab's pins or variables changed; the payload is its id.
const CONTEXT_EVENT: &str = "host://context";
/// Every tab's and side tab's shown status, whenever one changes (D15).
const STATUS_EVENT: &str = "host://status";
/// Each terminal pane's live label: path, running program, Claude's title (task 8.30).
const LABEL_EVENT: &str = "host://labels";
/// Saved statuses, rules or built-in looks changed; windows reload [`status_config`].
const STATUS_CONFIG_EVENT: &str = "host://status-config";
/// Saved commands changed; windows reload [`commands_list`].
const COMMANDS_EVENT: &str = "host://commands";
const SETTINGS_EVENT: &str = "host://settings";
const KEYS_EVENT: &str = "host://keys";
/// To one window: focus this pane, after `⌘⇧A` picked it (workbench-ui "Attention signals").
pub(crate) const REVEAL_EVENT: &str = "host://reveal-pane";
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

pub(crate) struct AppState {
    runtime: Runtime,
    db: Arc<Mutex<Db>>,
    attention: Arc<Mutex<Attention>>,
    /// The window whose Settings › Keys records a chord, while the menu holds no chords.
    recording: Mutex<Option<String>>,
    /// What text panes edit (D28).
    docs: Arc<terminescu_core::documents::Documents>,
    /// `terminescu edit --wait` calls waiting on their documents.
    waits: Arc<docs::Waits>,
    /// Side tab merges still inside their undo window, by undo id (D5).
    merges: Arc<Mutex<HashMap<Id, PendingMerge>>>,
    /// One merge or undo at a time: each reads the workspace, then changes it in two stores.
    merging: Mutex<()>,
    /// The tab drag the core tracks across windows (task 8.1).
    drags: drag::Drags,
}

/// How long a merge offers undo; the core keeps it a little longer for a late click.
const MERGE_UNDO: std::time::Duration = std::time::Duration::from_secs(12);

/// What undoing a merge needs: the context moves and where the source side tab stood.
struct PendingMerge {
    undo: merge::Undo,
    window_id: Id,
    index: usize,
    side_tab: SideTab,
    tab_ids: Vec<Id>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MergeDone {
    undo_id: Id,
    /// Source variables kept under a new name: old, new.
    renames: Vec<(String, String)>,
}

/// The variables that would clash if `from` merged into `into` (D5).
#[tauri::command(async)]
fn side_tab_merge_preview(
    state: State<'_, AppState>,
    from: Id,
    into: Id,
    label: Option<String>,
) -> Result<Vec<merge::Clash>, String> {
    let daemon = state.runtime.daemon();
    merge::preview(
        &*state.db,
        daemon.as_deref(),
        &from,
        &into,
        label.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// Merges side tab `from` into `into`: the context first, then the tabs. Undo stays open for
/// [`MERGE_UNDO`].
#[tauri::command(async)]
fn side_tab_merge(
    app: AppHandle,
    state: State<'_, AppState>,
    from: Id,
    into: Id,
    label: Option<String>,
    resolution: merge::Resolution,
) -> Result<MergeDone, String> {
    let _one = lock(&state.merging);
    let ws = state.runtime.snapshot().workspace;
    let (window_id, index, side_tab) = ws
        .windows
        .iter()
        .find_map(|w| {
            w.side_tabs
                .iter()
                .position(|s| s.id == from)
                .map(|i| (w.id.clone(), i, w.side_tabs[i].clone()))
        })
        .ok_or_else(|| format!("unknown side tab {from}"))?;
    if from == into || ws.side_tab(&into).is_none() {
        return Err(format!("unknown side tab {into}"));
    }
    let daemon = state.runtime.daemon();
    let undo = merge::merge(
        &*state.db,
        daemon.as_deref(),
        &from,
        &into,
        label.as_deref(),
        &resolution,
    )
    .map_err(|e| e.to_string())?;
    if let Err(e) = state.runtime.command(Command::MergeSideTab {
        from: from.clone(),
        into: into.clone(),
    }) {
        let _ = merge::undo(&*state.db, daemon.as_deref(), &undo);
        return Err(e.to_string());
    }
    let done = MergeDone {
        undo_id: undo.id.clone(),
        renames: undo.renames.clone(),
    };
    let tab_ids = side_tab.tabs.iter().map(|t| t.id.clone()).collect();
    lock(&state.merges).insert(
        undo.id.clone(),
        PendingMerge {
            undo,
            window_id,
            index,
            side_tab: SideTab {
                tabs: Vec::new(),
                ..side_tab
            },
            tab_ids,
        },
    );
    let (merges, rt, id) = (
        state.merges.clone(),
        state.runtime.clone(),
        done.undo_id.clone(),
    );
    let _ = std::thread::Builder::new()
        .name("merge-undo".into())
        .spawn(move || {
            std::thread::sleep(MERGE_UNDO);
            if let Some(p) = lock(&merges).remove(&id) {
                merge::finish(rt.daemon().as_deref(), &p.undo);
            }
        });
    merged(&app, &state, &into);
    Ok(done)
}

/// Undoes a merge inside its window: the source side tab comes back with the tabs that are
/// still in the target, and its context moves back.
#[tauri::command(async)]
fn side_tab_merge_undo(
    app: AppHandle,
    state: State<'_, AppState>,
    undo_id: Id,
) -> Result<(), String> {
    let _one = lock(&state.merging);
    let p = lock(&state.merges)
        .remove(&undo_id)
        .ok_or("too late to undo this merge")?;
    let daemon = state.runtime.daemon();
    // A refusal before anything changed keeps the undo on offer.
    let keep = |p: PendingMerge| {
        lock(&state.merges).insert(undo_id.clone(), p);
    };
    if let Err(e) = merge::can_undo(daemon.as_deref(), &p.undo) {
        keep(p);
        return Err(format!("the merge cannot be undone yet: {e}"));
    }
    let ws = state.runtime.snapshot().workspace;
    let tab_ids: Vec<Id> = p
        .tab_ids
        .iter()
        .filter(|t| ws.locate_tab(t).is_some_and(|(s, _)| s.id == p.undo.into))
        .cloned()
        .collect();
    let restored = state.runtime.command(Command::RestoreSideTab {
        window_id: p.window_id.clone(),
        index: p.index,
        side_tab: p.side_tab.clone(),
        tab_ids,
    });
    if let Err(e) = restored {
        merge::finish(daemon.as_deref(), &p.undo);
        return Err(format!("the merge cannot be undone: {e}"));
    }
    if let Err(e) = merge::undo(&*state.db, daemon.as_deref(), &p.undo) {
        // The context stayed merged, so the tabs go back to the target too.
        let back = state.runtime.command(Command::MergeSideTab {
            from: p.undo.from.clone(),
            into: p.undo.into.clone(),
        });
        if let Err(b) = back {
            eprintln!("terminescu: merge undo half done, side tab kept: {b}");
        } else {
            keep(p);
        }
        return Err(format!("the merge cannot be undone: {e}"));
    }
    merged(&app, &state, &p.undo.from);
    Ok(())
}

/// After a merge or its undo: contexts, local commands and statuses changed side tab.
fn merged(app: &AppHandle, state: &State<'_, AppState>, side: &str) {
    let db = lock(&state.db);
    match status_store::rules(&db) {
        Ok(rules) => state.runtime.set_status_rules(rules),
        Err(e) => eprintln!("terminescu: status rules not reloaded: {e}"),
    }
    refresh_launchers(&db, &state.runtime);
    drop(db);
    let _ = app.emit(CONTEXT_EVENT, side);
    let _ = app.emit(STATUS_CONFIG_EVENT, ());
    let _ = app.emit(COMMANDS_EVENT, ());
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
fn launcher_bytes(
    db: &impl DbAccess,
    rt: &Runtime,
    command: &SavedCommand,
) -> Result<Vec<u8>, String> {
    let side = command
        .side_tab_id
        .as_deref()
        .ok_or("a launcher command belongs to one side tab")?;
    let daemon = rt.daemon();
    let vars = vars::list(db, daemon.as_deref(), side).map_err(|e| e.to_string())?;
    let vars = rt.with_provided(side, vars);
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
    let bytes = launcher_bytes(db, rt, command)?;
    let ws = rt.workspace();
    let pane = command.tab_id.as_deref().and_then(|tab| {
        ws.windows
            .iter()
            .flat_map(|w| &w.side_tabs)
            .flat_map(|s| &s.tabs)
            .find(|t| t.id == tab)
            .and_then(|t| t.layout.terminals().into_iter().next().cloned())
    });
    let not_typed = || {
        format!(
            "{}: its tab has no shell right now; restart it",
            command.name
        )
    };
    if let Some(pane) = pane {
        return if rt.type_into(&pane, bytes) {
            Ok(())
        } else {
            Err(not_typed())
        };
    }
    let side = command.side_tab_id.clone().unwrap_or_default();
    let (pane, typed) = rt
        .command_then_type(Command::CreateTab { side_tab_id: side }, bytes)
        .map_err(|e| e.to_string())?;
    let tab = pane.and_then(|p| rt.workspace().locate_pane(&p).map(|(_, t)| t.id.clone()));
    // The tab stays the command's even when nothing was typed: it reads down, and restart
    // types into it.
    commands::set_tab(&lock(db), &command.id, tab.as_deref()).map_err(|e| e.to_string())?;
    if typed { Ok(()) } else { Err(not_typed()) }
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
/// Takes the database lock per step, never while typing.
fn rerun_launchers(db: &Mutex<Db>, rt: &Runtime) {
    let fresh = rt.take_respawned();
    if fresh.is_empty() {
        return;
    }
    let ws = rt.workspace();
    let list = commands::launchers(&lock(db)).unwrap_or_default();
    for c in list {
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
#[tauri::command(async)]
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
fn keys_get(state: State<'_, AppState>) -> Result<keys::Bindings, String> {
    keys::read(&lock(&state.db), &default_chords()).map_err(|e| e.to_string())
}

/// Saves the person's chords and rebuilds the menu, so the new chord wins at once (D17).
#[tauri::command]
fn keys_set(
    app: AppHandle,
    state: State<'_, AppState>,
    bindings: keys::Bindings,
) -> Result<keys::Bindings, String> {
    let saved =
        keys::set(&lock(&state.db), &default_chords(), bindings).map_err(|e| e.to_string())?;
    let menu = build_menu(&app, &saved, true).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    *lock(&state.recording) = None;
    let _ = app.emit(KEYS_EVENT, ());
    Ok(saved)
}

/// Builds the menu from the saved chords, with them or without them.
pub(crate) fn set_chords(app: &AppHandle, db: &Mutex<Db>, chords: bool) -> Result<(), String> {
    let bindings = keys::read(&lock(db), &default_chords()).map_err(|e| e.to_string())?;
    let menu = build_menu(app, &bindings, chords).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
}

/// Takes the action chords off the menu while Settings › Keys records one, and puts them back.
#[tauri::command]
fn keys_record(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    set_chords(&app, &state.db, !on)?;
    *lock(&state.recording) = on.then(|| window.label().to_string());
    Ok(())
}

/// The recording window reloaded or closed before it said it stopped: the chords come back.
fn end_recording(app: &AppHandle, label: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let mut rec = lock(&state.recording);
    if rec.as_deref() == Some(label) {
        *rec = None;
        drop(rec);
        if let Err(e) = set_chords(app, &state.db, true) {
            eprintln!("terminescu: shortcuts not restored: {e}");
        }
    }
}

#[tauri::command]
fn commands_all(state: State<'_, AppState>) -> Result<Vec<SavedCommand>, String> {
    commands::all(&lock(&state.db)).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Result<settings::Settings, String> {
    settings::get(&lock(&state.db)).map_err(|e| e.to_string())
}

/// Adds or removes the plugins' helpers for a new answer; `Unasked` leaves them alone.
fn apply_hook(consent: settings::HookConsent) -> Result<(), String> {
    let failed = terminescu_core::helpers::apply(&plugins::helpers(), consent);
    terminescu_core::helpers::describe(&failed).map_or(Ok(()), Err)
}

#[tauri::command]
fn settings_set(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: settings::Settings,
) -> Result<settings::Settings, String> {
    let before = settings::get(&lock(&state.db))
        .map_err(|e| e.to_string())?
        .hook;
    // The hook toggle (D5): Claude Code's settings change first, so a failed edit saves nothing.
    if settings.hook != before {
        apply_hook(settings.hook)?;
    }
    let saved = settings::set(&lock(&state.db), settings).map_err(|e| e.to_string())?;
    state.runtime.set_editor(saved.editor);
    state.runtime.set_scrollback(saved.terminal_scrollback);
    for w in app.webview_windows().values() {
        let _ = w.set_zoom(zoom_of(&saved));
    }
    let _ = app.emit(SETTINGS_EVENT, ());
    Ok(saved)
}

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
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

/// "Send to Tab…" (D13): text into another tab's terminal as a paste, never with Enter.
#[tauri::command(async)]
fn pane_send_text(state: State<'_, AppState>, pane: Id, text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }
    if !state
        .runtime
        .type_into(&pane, commands::type_bytes(&text, false))
    {
        return Err("that pane has no shell right now".into());
    }
    Ok(())
}

/// Runs (`run`) or inserts a saved command from a side tab, filled from that side tab's
/// variables and `extra`. `target` overrides the saved one; `pane` is the focused pane.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
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
    let command = commands::get(&lock(&state.db), &id).map_err(|e| e.to_string())?;
    let daemon = state.runtime.daemon();
    let vars = vars::list(&*state.db, daemon.as_deref(), &side_tab).map_err(|e| e.to_string())?;
    let vars = state.runtime.with_provided(&side_tab, vars);
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
        Target::NewTab | Target::NewSideTab => {
            let cmd = match target.unwrap_or(command.target) {
                Target::NewTab => Command::CreateTab {
                    side_tab_id: side_tab,
                },
                _ => Command::CreateSideTab {
                    window_id,
                    index: None,
                },
            };
            let (pane, typed) = rt
                .command_then_type(cmd, bytes)
                .map_err(|e| e.to_string())?;
            if pane.is_some() && !typed {
                return Err("the new tab has no shell yet; run the command there again".into());
            }
            pane
        }
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

/// Posts a notification when a command that ran at least the threshold ends while no window has
/// focus (task 8.40). A pane where Claude runs is left to the waiting notification.
fn watch_commands(app: AppHandle, runtime: Runtime, db: Arc<Mutex<Db>>) {
    use tauri_plugin_notification::NotificationExt;
    let (tx, ends) = std::sync::mpsc::channel::<terminescu_core::runtime::CommandEnd>();
    runtime.on_command_end(Box::new(move |end| {
        let _ = tx.send(end);
    }));
    std::thread::spawn(move || {
        for end in ends {
            let settings = attention::settings(&lock(&db)).unwrap_or_default();
            if !settings.command_notify || end.ms < u64::from(settings.command_secs) * 1000 {
                continue;
            }
            let focused = app
                .webview_windows()
                .values()
                .any(|w| w.is_focused().unwrap_or(false));
            let claude = runtime
                .agents()
                .iter()
                .any(|a| a.pane == end.pane && a.agent.is_some());
            if focused || claude {
                continue;
            }
            let ws = runtime.snapshot().workspace;
            let Some(place) = attention::place_of(&ws, &end.pane) else {
                continue;
            };
            let what = end
                .command
                .as_deref()
                .and_then(attention::command_label)
                .map(|c| format!("`{c}`"))
                .unwrap_or_else(|| "A command".into());
            let took = attention::run_time(end.ms);
            let body = match end.exit {
                Some(code) if code != 0 => format!("{what} failed with exit {code} after {took}"),
                _ => format!("{what} finished in {took}"),
            };
            let shown = app
                .notification()
                .builder()
                .title(format!("{} › {}", place.side_name, place.tab_name))
                .body(body)
                .show();
            if let Err(e) = shown {
                eprintln!("terminescu: notification not posted: {e}");
            }
        }
    });
}

/// Feeds agent updates into [`Attention`] off the runtime's locks: the badge follows the count,
/// and a pane that starts waiting bounces the dock icon, posts a notification while no window
/// has focus, and the chime when it is on. The notification names the side tab and tab and plays no sound.
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
            // The dock icon bounces once; macOS ignores it while the app is in front.
            if let Some(w) = windows.get(place.window.as_str())
                && let Err(e) =
                    w.request_user_attention(Some(tauri::UserAttentionType::Informational))
            {
                eprintln!("terminescu: dock bounce not requested: {e}");
            }
            if settings.notify && !focused {
                let shown = app
                    .notification()
                    .builder()
                    .title(format!("{} › {}", place.side_name, place.tab_name))
                    // The adapter names what the agent waits on (add-claude-insights 2.3).
                    .body(
                        u.agent
                            .as_ref()
                            .and_then(|a| a.reason.as_deref())
                            .unwrap_or("Waiting for you"),
                    )
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
fn host_agents(state: State<'_, AppState>) -> Vec<AgentUpdate> {
    state.runtime.agents()
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
/// its value (D21). Plugin variables come last, read-only, under names not taken.
#[tauri::command(async)]
fn vars_list(
    state: State<'_, AppState>,
    side_tab: String,
) -> Result<Vec<context::Variable>, String> {
    let daemon = state.runtime.daemon();
    let saved = vars::list(&*state.db, daemon.as_deref(), &side_tab).map_err(|e| e.to_string())?;
    Ok(state.runtime.with_provided(&side_tab, saved))
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
    let v = vars::add(&*state.db, daemon.as_deref(), &side_tab, &variable, replace)
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
    let v = vars::change(&*state.db, daemon.as_deref(), &side_tab, &name, &change)
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
    vars::remove(&*state.db, daemon.as_deref(), &side_tab, &name).map_err(|e| e.to_string())?;
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

#[tauri::command(async)]
fn files_touched(
    state: State<'_, AppState>,
    side_tab: String,
) -> Result<Vec<terminescu_core::files::TouchedFile>, String> {
    terminescu_core::files::list(&lock(&state.db), &side_tab).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn files_forget(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let side = terminescu_core::files::remove(&lock(&state.db), &id).map_err(|e| e.to_string())?;
    if let Some(side) = side {
        let _ = app.emit(CONTEXT_EVENT, side);
    }
    Ok(())
}

#[tauri::command]
fn host_statuses(state: State<'_, AppState>) -> BTreeMap<Id, Shown> {
    state.runtime.statuses()
}

#[tauri::command]
fn host_labels(state: State<'_, AppState>) -> BTreeMap<Id, PaneLabel> {
    state.runtime.labels()
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
    // These move contexts too; they go through `side_tab_merge` and its undo, or
    // `reopen_closed`.
    if let Command::MergeSideTab { from, .. }
    | Command::RestoreSideTab {
        side_tab: SideTab { id: from, .. },
        ..
    }
    | Command::ReopenSideTab {
        side_tab: SideTab { id: from, .. },
        ..
    }
    | Command::ReopenTab {
        side_tab_id: from, ..
    } = &command
    {
        return Err(CommandError::UnknownSideTab(from.clone()));
    }
    // A close keeps what it takes for ⇧⌘T (task 8.87).
    let noted = reopen::note(&state, &command);
    // The runtime emits each batch under its core lock, so windows see them in rev order.
    let done = state.runtime.command(command);
    if let Some(token) = noted {
        reopen::done(&state, token, done.is_ok());
    }
    done
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

/// The terminal cleared its history or cleared to its prompt (task 8.44).
#[tauri::command]
fn pane_clear(state: State<'_, AppState>, view: ViewId, keep_screen: bool) {
    state.runtime.view_clear(view, keep_screen);
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

/// Opens a link from a pane: http(s) in the default browser, a `mailto:` address in Mail
/// (task 8.49). Nothing else leaves the app, so the url can never read as an `open` option.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://") || mail_ok(&url)) {
        return Err(format!("not an http(s) or mail link: {url}"));
    }
    std::process::Command::new("/usr/bin/open")
        .arg(&url)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// `mailto:` and one plain address: no spaces, headers or second recipient.
fn mail_ok(url: &str) -> bool {
    let Some(addr) = url.strip_prefix("mailto:") else {
        return false;
    };
    let Some((user, host)) = addr.split_once('@') else {
        return false;
    };
    let ok = |s: &str, extra: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
    };
    ok(user, "._%+-") && ok(host, ".-") && host.contains('.') && !user.starts_with('-')
}

/// Bounces the dock icon once for a terminal's bell (task 8.42); macOS ignores it while the app
/// is in front.
#[tauri::command]
fn bounce_dock(window: tauri::WebviewWindow) -> Result<(), String> {
    window
        .request_user_attention(Some(tauri::UserAttentionType::Informational))
        .map_err(|e| e.to_string())
}

/// Settings › Terminal › Import from Terminal.app (task 8.47): its default profile.
#[tauri::command]
async fn import_terminal_app() -> Result<term_import::Profile, String> {
    term_import::terminal_app_default().map_err(|e| e.to_string())
}

/// Settings › Terminal › Import file: a `.terminal` profile or an `.itermcolors` preset.
#[tauri::command]
async fn import_terminal_file(path: PathBuf) -> Result<term_import::Profile, String> {
    term_import::import_file(&path).map_err(|e| e.to_string())
}

/// The sidecars sit next to the app binary: in `Contents/MacOS` when bundled, in
/// `target/debug` under `tauri dev`.
fn sidecar(name: &str) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let path = exe.parent().unwrap_or(Path::new(".")).join(name);
    if !path.is_file() {
        // A bundle built without tauri.bundle.conf.json lacks the sidecars.
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{name} is missing from {}. Build the app with `pnpm tauri:build`.",
                path.parent().unwrap_or(Path::new(".")).display()
            ),
        ));
    }
    Ok(path)
}

/// Installs the plugins' helpers when the person agreed; one already installed counts as
/// consent and ends the first run, as on a Mac that had Terminescu before consent (D5, D6).
fn hook_at_launch(db: &Arc<Mutex<Db>>) {
    let db = lock(db);
    let Ok(mut s) = settings::get(&db) else {
        return;
    };
    let (now, failed) = terminescu_core::helpers::at_launch(&plugins::helpers(), s.hook);
    for (id, e) in failed {
        eprintln!("terminescu: helper {id} not installed: {e}");
    }
    if now != s.hook {
        s.hook = now;
        s.first_run_done = true;
        if let Err(e) = settings::set(&db, s) {
            eprintln!("terminescu: hook consent not saved: {e}");
        }
    }
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
        // Claude Code runs the hook from the copy, whose path survives updates (D36), and only
        // once the person agreed (add-public-release D5).
        Ok(_) => hook_at_launch(&db),
        Err(e) => eprintln!("terminescu: CLI copy failed: {e}"),
    }
    let run = paths::run_paths();
    paths::ensure_private_dir(&run.dir)?;
    let hooks: Arc<OnceLock<Runtime>> = Arc::default();
    if let Err(e) = serve_core_socket(&run.core_socket, hooks.clone(), db.clone(), app.clone()) {
        eprintln!("terminescu: core socket not served, hooks and statuses stay off: {e}");
    }
    let log = paths::support_dir().join("logs/ptyd.log");
    let handle = app.clone();
    let emit = Box::new(move |batch: &terminescu_core::state::PatchBatch| {
        if let Err(e) = handle.emit(PATCHES_EVENT, batch) {
            eprintln!("terminescu: patch broadcast failed: {e}");
        }
        // Native windows follow the workspace (task 8.1). This runs under the core lock, so the
        // window work goes to one thread per batch, in patch order: the new window exists before
        // an emptied one closes, so the app never sees zero windows and quits.
        let work: Vec<Patch> = batch
            .patches
            .iter()
            .filter(|p| matches!(p, Patch::WindowCreated { .. } | Patch::WindowClosed { .. }))
            .cloned()
            .collect();
        if !work.is_empty() {
            let app = handle.clone();
            let at = drag::take_window_at();
            std::thread::spawn(move || {
                for p in work {
                    match p {
                        Patch::WindowCreated { window_id } => {
                            if let Err(e) = open_window(&app, &window_id, at) {
                                eprintln!("terminescu: window {window_id} not opened: {e}");
                            }
                        }
                        Patch::WindowClosed { window_id } => {
                            drag::cancel_from(&app, &window_id);
                            if let Some(w) = app.get_webview_window(&window_id) {
                                let _ = w.destroy();
                            }
                        }
                        _ => {}
                    }
                }
            });
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
    let (mut saved, rules) = {
        let db = lock(&db);
        (persist::load(&db)?, status_store::rules(&db)?)
    };
    // Rows and tabs saved before neons existed get one now, kept by the next save (8.26).
    if let Some(s) = saved.as_mut() {
        terminescu_core::state::fill_accents(&mut s.workspace);
    }
    let (editor, scrollback) = {
        let db = lock(&db);
        settings::get(&db)
            .map(|s| (s.editor, s.terminal_scrollback))
            .unwrap_or((false, settings::SCROLLBACK))
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
    let found_db = db.clone();
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
                // A closed row ⇧⌘T can reopen keeps them until it leaves the list.
                for side in gone.iter().filter(|s| !reopen::is_parked(s)) {
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
        editor,
    )?;
    runtime.set_scrollback(scrollback);
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
    runtime.on_labels(Box::new(move |map| {
        if let Err(e) = handle.emit(LABEL_EVENT, map) {
            eprintln!("terminescu: label broadcast failed: {e}");
        }
    }));
    let handle = app.clone();
    runtime.on_context(Box::new(move |side: &str| {
        let _ = handle.emit(CONTEXT_EVENT, side);
    }));
    let found_app = app.clone();
    runtime.on_found(Box::new(move |side: &str, tab: &str, found| {
        match terminescu_core::extract::record(&lock(&found_db), side, Some(tab), &found) {
            Ok(0) => {}
            Ok(_) => {
                let _ = found_app.emit(CONTEXT_EVENT, side);
            }
            Err(e) => eprintln!("terminescu: saving extracted items: {e}"),
        }
    }));
    runtime.set_status_rules(rules);
    let _ = hooks.set(runtime.clone());
    Ok((runtime, windows, updates))
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn side_ids(ws: &Workspace) -> std::collections::HashSet<Id> {
    ws.windows
        .iter()
        .flat_map(|w| w.side_tabs.iter().map(|s| s.id.clone()))
        .collect()
}

/// Answers `terminescu hook`, `status`, saved `var set` and `edit` (D27, D34, D36).
fn serve_core_socket(
    path: &Path,
    runtime: Arc<OnceLock<Runtime>>,
    db: Arc<Mutex<Db>>,
    app: AppHandle,
) -> std::io::Result<()> {
    // Hook events change agent state and write to the database, which can wait on a lock;
    // `terminescu hook` must not wait for that.
    let (hooks_tx, hooks_rx) =
        std::sync::mpsc::channel::<(terminescu_protocol::PaneId, serde_json::Value)>();
    {
        let (runtime, db) = (runtime.clone(), db.clone());
        std::thread::Builder::new()
            .name("hook-events".into())
            .spawn(move || {
                for (pane, event) in hooks_rx {
                    let Some(rt) = runtime.get() else { continue };
                    // A file the agent wrote or edited lands in the side tab's Files (D30).
                    let touched = rt.hook(pane, &event);
                    let Some((side, tab)) = (!touched.is_empty())
                        .then(|| rt.place_of_daemon(pane))
                        .flatten()
                    else {
                        continue;
                    };
                    let db = lock(&db);
                    let recorded = touched
                        .iter()
                        .filter(|t| {
                            terminescu_core::files::record(&db, &side, Some(&tab), t).is_ok()
                        })
                        .count();
                    drop(db);
                    if recorded > 0 {
                        rt.context_changed(&side);
                    }
                }
            })?;
    }
    core_socket::serve(
        path,
        Arc::new(
            move |message: CoreMessage, caller: &core_socket::Caller| match message.request {
                // The session a hook names drives the tab's agent state (task 4.2) and the resume
                // offer (task 3.4).
                CoreRequest::Hook { event } => {
                    // Replies at once; the hook thread does the work in arrival order (D34).
                    let _ = hooks_tx.send((message.pane, event));
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
                    match vars::add(&*db, daemon.as_deref(), &side, &new, true) {
                        Ok(_) => {
                            rt.context_changed(&side);
                            CoreReply::Ok
                        }
                        Err(e) => CoreReply::error("bad_var", e.to_string()),
                    }
                }
                // `terminescu edit`; with `--wait` this call thread blocks until the pane returns.
                CoreRequest::Edit { path, wait } => {
                    docs::edit_from_cli(&app, message.pane, &path, wait, caller)
                }
            },
        ),
    )
}

/// Opens the native window of a workspace window. `at` is a drop point in screen pixels with
/// that screen's scale: the window opens hidden, moves under it so the cursor lands near its
/// tab bar, then shows. Physical pixels keep screens left of or above the main one right.
/// The webview zoom for the saved percent (task 8.8): native page zoom, so xterm.js sees a new
/// pixel ratio and refits like any resize.
fn zoom_of(s: &settings::Settings) -> f64 {
    f64::from(s.zoom_percent) / 100.0
}

/// A new window's size in points.
const WINDOW_SIZE: (f64, f64) = (1200.0, 760.0);
/// How far right and down a new window opens from the focused one, in points (task 8.82).
const CASCADE: f64 = 28.0;

/// Where a window of `size` opens below and right of one at `from`, as Terminal.app cascades.
/// When it would leave the screen's work `area` (x, y, width, height), it starts again at the
/// area's top left. All in points.
fn cascade(from: (f64, f64), size: (f64, f64), area: (f64, f64, f64, f64)) -> (f64, f64) {
    let (x, y) = (from.0 + CASCADE, from.1 + CASCADE);
    let (ax, ay, aw, ah) = area;
    if x + size.0 > ax + aw || y + size.1 > ay + ah {
        (ax + CASCADE, ay + CASCADE)
    } else {
        (x, y)
    }
}

/// The cascaded spot for a new window, from the focused Terminescu window; none when no
/// window has the focus, as at launch.
fn cascade_spot(app: &AppHandle, new_id: &str) -> Option<(f64, f64)> {
    let w = app.webview_windows().into_values().find(|w| {
        w.label() != new_id && w.label() != drag::GHOST && w.is_focused().unwrap_or(false)
    })?;
    let scale = w.scale_factor().ok()?;
    let at = w.outer_position().ok()?.to_logical::<f64>(scale);
    let monitor = w.current_monitor().ok()??;
    let wa = monitor.work_area();
    let ms = monitor.scale_factor();
    let area = (
        f64::from(wa.position.x) / ms,
        f64::from(wa.position.y) / ms,
        f64::from(wa.size.width) / ms,
        f64::from(wa.size.height) / ms,
    );
    Some(cascade((at.x, at.y), WINDOW_SIZE, area))
}

/// A frame as (x, y, width, height) in points.
type Rect = (f64, f64, f64, f64);

/// The height of the strip a window is dragged by, and how much of it must lie on a screen.
const TITLE_STRIP: f64 = 34.0;
const ON_SCREEN: f64 = 100.0;

/// Where a saved frame reopens (task 8.85). A frame whose title strip still lies on one of the
/// screens' work areas keeps its place; one off every screen, as after a screen goes, moves to
/// the main screen's top left and shrinks to fit it. All in points.
fn fit_frame(frame: Rect, screens: &[Rect], main: Rect) -> Rect {
    let (x, y, w, h) = frame;
    let reachable = screens.iter().any(|&(sx, sy, sw, sh)| {
        let across = (x + w).min(sx + sw) - x.max(sx);
        let down = (y + TITLE_STRIP).min(sy + sh) - y.max(sy);
        across >= ON_SCREEN && down >= TITLE_STRIP / 2.0
    });
    if reachable {
        return frame;
    }
    let (mx, my, mw, mh) = main;
    (
        mx + CASCADE,
        my + CASCADE,
        w.min(mw - 2.0 * CASCADE).max(640.0),
        h.min(mh - 2.0 * CASCADE).max(400.0),
    )
}

/// A monitor's work area in points.
fn work_area(m: &tauri::Monitor) -> Rect {
    let (wa, s) = (m.work_area(), m.scale_factor());
    (
        f64::from(wa.position.x) / s,
        f64::from(wa.position.y) / s,
        f64::from(wa.size.width) / s,
        f64::from(wa.size.height) / s,
    )
}

/// The saved frame of a window, fitted to the screens there are now.
fn saved_frame(app: &AppHandle, window_id: &str) -> Option<Rect> {
    let f = persist::window_frame(&lock(&app.state::<AppState>().db), window_id).ok()??;
    let screens: Vec<Rect> = app
        .available_monitors()
        .ok()?
        .iter()
        .map(work_area)
        .collect();
    let main = app
        .primary_monitor()
        .ok()?
        .map(|m| work_area(&m))
        .or(screens.first().copied())?;
    Some(fit_frame((f.x, f.y, f.width, f.height), &screens, main))
}

/// Each window's latest move or resize; a save waits until it is 500 ms old (task 8.85). A
/// window with a save waiting has an entry; its one thread sleeps until the last move is old.
static FRAME_MOVES: OnceLock<Mutex<HashMap<String, std::time::Instant>>> = OnceLock::new();

const FRAME_SETTLE: Duration = Duration::from_millis(500);

fn frame_moved(app: &AppHandle, label: &str) {
    let moves = FRAME_MOVES.get_or_init(Default::default);
    if lock(moves)
        .insert(label.to_string(), std::time::Instant::now())
        .is_some()
    {
        return;
    }
    let (app, label) = (app.clone(), label.to_string());
    std::thread::spawn(move || {
        let mut wait = FRAME_SETTLE;
        loop {
            std::thread::sleep(wait);
            let mut m = lock(moves);
            match m.get(&label).map(|last| last.elapsed()) {
                Some(age) if age < FRAME_SETTLE => wait = FRAME_SETTLE - age,
                _ => {
                    m.remove(&label);
                    break;
                }
            }
        }
        let Some(w) = app.get_webview_window(&label) else {
            return;
        };
        // A full-screen or minimised window keeps the frame it had before.
        if w.is_fullscreen().unwrap_or(false) || w.is_minimized().unwrap_or(false) {
            return;
        }
        let (Ok(scale), Ok(at), Ok(size)) = (w.scale_factor(), w.outer_position(), w.inner_size())
        else {
            return;
        };
        let (at, size) = (at.to_logical::<f64>(scale), size.to_logical::<f64>(scale));
        let frame = persist::WindowFrame {
            x: at.x,
            y: at.y,
            width: size.width,
            height: size.height,
        };
        if let Some(state) = app.try_state::<AppState>()
            && let Err(e) = persist::save_window_frame(&lock(&state.db), &label, &frame)
        {
            eprintln!("window frame not saved: {e}");
        }
    });
}

fn open_window(app: &AppHandle, window_id: &str, at: Option<(f64, f64, f64)>) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, window_id, WebviewUrl::App("index.html".into()))
        .title("Terminescu")
        .inner_size(WINDOW_SIZE.0, WINDOW_SIZE.1)
        .min_inner_size(640.0, 400.0)
        .visible(at.is_none());
    // A drag's drop point wins, then the window's saved frame (task 8.85), then the cascade
    // from the focused window (task 8.82).
    let builder = match at {
        Some(_) => builder,
        None => match saved_frame(app, window_id) {
            Some((x, y, w, h)) => builder.position(x, y).inner_size(w, h),
            None => match cascade_spot(app, window_id) {
                Some((x, y)) => builder.position(x, y),
                None => builder,
            },
        },
    };
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        // Centres the lights in the 34 px sidebar strip that holds the wordmark (app.css).
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 20.0))
        // The click that brings the window forward also reaches the page, so a double click
        // on an icon or a name works from another app (task 8.19).
        .accept_first_mouse(true)
        // Terminal opacity (task 8.47): the page paints its own background, opaque at 100%.
        .transparent(true);
    let w = builder.build()?;
    // The caller holds no db lock: the window opens from the patch emitter and from setup.
    let saved = settings::get(&lock(&app.state::<AppState>().db)).unwrap_or_default();
    if saved.zoom_percent != 100 {
        w.set_zoom(zoom_of(&saved))?;
    }
    if let Some((x, y, scale)) = at {
        w.set_position(tauri::PhysicalPosition::new(
            x - 120.0 * scale,
            y - 16.0 * scale,
        ))?;
        w.show()?;
        w.set_focus()?;
    }
    Ok(())
}

fn actions_file() -> ActionsFile {
    serde_json::from_str(ACTIONS_JSON).expect("actions.json is checked by the host tests")
}

/// Each action's id and default chord, for the checks in `keys`.
fn default_chords() -> Vec<(String, String)> {
    actions_file()
        .actions
        .into_iter()
        .map(|a| (a.id, a.accel))
        .collect()
}

/// The app menu (D17): the standard macOS menus plus every action from actions.json under
/// its menu, with its chord or the one the person set in Settings › Keys.
/// `chords: false` builds it with no action chords, while Settings › Keys records one: macOS
/// gives a menu chord to the menu before the window sees the key.
fn build_menu(
    app: &AppHandle,
    bindings: &keys::Bindings,
    chords: bool,
) -> tauri::Result<Menu<Wry>> {
    let file = actions_file();
    let mut sections: Vec<(String, Vec<MenuItem<Wry>>)> = Vec::new();
    for a in &file.actions {
        let accel = bindings.get(&a.id).unwrap_or(&a.accel);
        // An empty chord is off (add-configurable-shortcuts D2).
        let accel = (chords && !accel.is_empty()).then_some(accel.as_str());
        let item = MenuItem::with_id(app, &a.id, &a.label, true, accel)?;
        match sections.iter_mut().find(|(name, _)| *name == a.menu) {
            Some((_, items)) => items.push(item),
            None => sections.push((a.menu.clone(), vec![item])),
        }
    }
    // Plugin actions join the menu they name, or a menu of their own (add-plugin-architecture D7).
    for a in plugins::menu_items(app) {
        let accel = (chords && !a.accel.is_empty()).then_some(a.accel.as_str());
        let item = MenuItem::with_id(app, &a.id, &a.label, true, accel)?;
        match sections.iter_mut().find(|(name, _)| *name == a.menu) {
            Some((_, items)) => items.push(item),
            None => sections.push((a.menu, vec![item])),
        }
    }
    let edit = Submenu::with_items(
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
        ],
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                "Terminescu",
                true,
                &[
                    // The app's own About (add-public-release D7): it shows the maker, the
                    // build and the license, which the standard panel cannot.
                    &MenuItem::with_id(app, public::ABOUT, "About Terminescu", true, None::<&str>)?,
                    &MenuItem::with_id(
                        app,
                        public::CHECK_UPDATES,
                        "Check for Updates…",
                        true,
                        None::<&str>,
                    )?,
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
            &edit,
        ],
    )?;
    for (name, items) in &sections {
        let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items
            .iter()
            .map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect();
        // Find, clear and the prompt marks join the standard Edit menu, under a separator.
        // Select All is the app's own item under Paste: macOS's would select only inside the
        // terminal's hidden input, so the app selects the terminal's whole buffer instead.
        if name == "Edit" {
            let (all, rest): (Vec<_>, Vec<_>) = items
                .iter()
                .zip(refs)
                .partition(|(i, _)| i.id().as_ref() == "select_all");
            for (_, r) in all {
                edit.append(r)?;
            }
            edit.append(&PredefinedMenuItem::separator(app)?)?;
            edit.append_items(&rest.into_iter().map(|(_, r)| r).collect::<Vec<_>>())?;
            continue;
        }
        let sub = Submenu::with_items(app, name, true, &refs)?;
        if name == "Window" {
            sub.prepend_items(&[
                &PredefinedMenuItem::minimize(app, None)?,
                &PredefinedMenuItem::fullscreen(app, None)?,
                &PredefinedMenuItem::separator(app)?,
            ])?;
            sub.set_as_windows_menu_for_nsapp()?;
        }
        // File sits between the app menu and Edit, as macOS apps place it.
        if name == "File" {
            menu.insert(&sub, 1)?;
        } else {
            menu.append(&sub)?;
        }
    }
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[
            &item(public::FEEDBACK_EMAIL, "Send Feedback…")?,
            &item(public::FEEDBACK_ISSUE, "Report an Issue…")?,
            &PredefinedMenuItem::separator(app)?,
            &item(public::PRIVACY, "Privacy")?,
            &item(public::LICENSES, "Licenses")?,
            &PredefinedMenuItem::separator(app)?,
            &item(public::UNINSTALL, "Uninstall Terminescu Helpers…")?,
        ],
    )?;
    help.set_as_help_menu_for_nsapp()?;
    menu.append(&help)?;
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

/// Opens the database, starts the runtime and opens the saved windows. An error here ends the
/// start: `main` logs it and shows it in an alert.
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db = Db::open(dir.join("terminescu.sqlite"))?;
    db.local_owner()?;
    updates::start(app.handle());
    if let Err(e) = prompts::import_seeds(&db, prompts::SEEDS) {
        eprintln!("terminescu: seed prompts not imported: {e}");
    }
    if let Err(e) = status_store::seed_defaults(&db) {
        eprintln!("terminescu: default statuses not added: {e}");
    }
    let db = Arc::new(Mutex::new(db));
    let (runtime, windows, updates) = start_runtime(app.handle(), db.clone())?;
    let waits = Arc::new(docs::Waits::default());
    let docs = docs::start(app.handle(), db.clone(), &runtime, waits.clone());
    let attention = Arc::new(Mutex::new(Attention::default()));
    watch_attention(
        app.handle().clone(),
        runtime.clone(),
        db.clone(),
        attention.clone(),
        updates,
    );
    watch_commands(app.handle().clone(), runtime.clone(), db.clone());
    {
        refresh_launchers(&lock(&db), &runtime);
        rerun_launchers(&db, &runtime);
        let db = lock(&db);
        // Chords from Settings › Keys; each one that no longer passes the checks (an action
        // renamed, a new default that clashes) falls back to its default alone.
        let bindings = keys::read(&db, &default_chords()).unwrap_or_else(|e| {
            eprintln!("terminescu: saved shortcuts not read: {e}");
            keys::Bindings::new()
        });
        if !bindings.is_empty() {
            app.set_menu(build_menu(app.handle(), &bindings, true)?)?;
        }
    }
    let plugins = Arc::new(plugins::Plugins::start(app.handle(), db.clone()));
    plugins::observe(app.handle(), &runtime, plugins.clone());
    plugins.dispatch(&terminescu_plugin_api::CoreEvent::AppLaunched);
    app.manage(plugins);
    app.manage(AppState {
        runtime,
        db,
        attention,
        recording: Mutex::new(None),
        docs,
        waits,
        merges: Arc::default(),
        merging: Mutex::default(),
        drags: drag::Drags::default(),
    });
    for id in &windows {
        open_window(app.handle(), id, None)?;
    }
    Ok(())
}

fn main() {
    crashlog::init();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Pending::default())
        .menu(|app| build_menu(app, &keys::Bindings::new(), true))
        .on_menu_event(|app, event| send_action(app, event.id().as_ref()))
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                end_recording(webview.app_handle(), webview.label());
            }
        })
        .setup(|app| {
            setup(app).unwrap_or_else(|e| crashlog::fail(&e));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            edit::edit_command,
            plugins::plugin_call,
            plugins::plugin_menu,
            plugins::helpers_list,
            plugins::plugin_row_selected,
            plugins::plugin_list,
            plugins::plugin_set_enabled,
            plugins::decorations,
            public::app_info,
            public::feedback_open,
            public::uninstall_helpers,
            public::app_quit,
            updates::update_check,
            updates::update_restart,
            host_snapshot,
            host_agents,
            pins_list,
            pin_add,
            pin_remove,
            vars_list,
            var_add,
            var_change,
            var_remove,
            items_list,
            files_touched,
            pane_send_text,
            side_tab_merge_preview,
            side_tab_merge,
            side_tab_merge_undo,
            drag::drag_begin,
            drag::drag_end,
            drag::drag_title,
            files_forget,
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
            keys_get,
            keys_set,
            keys_record,
            command_create,
            command_update,
            command_delete,
            command_run,
            launcher_restart,
            attention_settings,
            attention_set,
            host_statuses,
            host_labels,
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
            pane_clear,
            import_terminal_app,
            import_terminal_file,
            open_url,
            bounce_dock,
            docs::doc_new,
            docs::doc_open_file,
            docs::doc_open_path,
            docs::path_open,
            docs::doc_open_prompt,
            docs::doc_open_variable,
            docs::doc_get,
            docs::doc_edit,
            docs::doc_save,
            docs::doc_reload,
            docs::doc_keep_mine,
            docs::doc_return,
            docs::doc_paste_image,
            docs::files_list,
            reopen::reopen_closed
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| crashlog::fail(&e));
    app.run(|app, event| {
        if let RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_),
            ..
        } = &event
            && label != drag::GHOST
        {
            frame_moved(app, label);
        }
        if let RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Destroyed,
            ..
        } = &event
        {
            end_recording(app, label);
            // The hidden ghost must not keep the app alive once the last window is gone.
            let left = app.webview_windows().into_keys().any(|l| l != drag::GHOST);
            if !left && let Some(g) = app.get_webview_window(drag::GHOST) {
                let _ = g.destroy();
            }
        }
        // A last save with fresh cwds, so a reboot after quitting restarts shells where they were.
        if let RunEvent::Exit = event
            && let Some(state) = app.try_state::<AppState>()
        {
            // Merges still inside their undo window end now: their parked secrets go.
            let daemon = state.runtime.daemon();
            for (_, p) in lock(&state.merges).drain() {
                merge::finish(daemon.as_deref(), &p.undo);
            }
            reopen::end(&state);
            state.runtime.stop();
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn mail_links_take_one_plain_address() {
        assert!(super::mail_ok("mailto:dev@example.com"));
        assert!(!super::mail_ok("mailto:dev@example.com?bcc=x@example.com"));
        assert!(!super::mail_ok("mailto:a b@example.com"));
        assert!(!super::mail_ok("mailto:-a@example.com"));
        assert!(!super::mail_ok("dev@example.com"));
    }

    use super::*;

    #[test]
    fn a_new_window_cascades_and_wraps_at_the_screen_edge() {
        let area = (0.0, 25.0, 1728.0, 1080.0);
        assert_eq!(cascade((100.0, 60.0), WINDOW_SIZE, area), (128.0, 88.0));
        // 528 + 1200 fits a 1728-point screen exactly; one more step would not.
        assert_eq!(cascade((500.0, 60.0), WINDOW_SIZE, area), (528.0, 88.0));
        assert_eq!(cascade((501.0, 60.0), WINDOW_SIZE, area), (28.0, 53.0));
        assert_eq!(cascade((100.0, 400.0), WINDOW_SIZE, area), (28.0, 53.0));
    }

    #[test]
    fn a_saved_frame_fits_the_screens() {
        let laptop = (0.0, 25.0, 1728.0, 1080.0);
        let external = (1728.0, 0.0, 2560.0, 1415.0);
        let on_external = (2000.0, 200.0, 1400.0, 900.0);
        assert_eq!(
            fit_frame(on_external, &[laptop, external], laptop),
            on_external
        );
        // The external screen is gone: the frame moves to the laptop and shrinks to fit.
        assert_eq!(
            fit_frame(on_external, &[laptop], laptop),
            (28.0, 53.0, 1400.0, 900.0)
        );
        let tall = (2000.0, 100.0, 1600.0, 1300.0);
        assert_eq!(
            fit_frame(tall, &[laptop], laptop),
            (28.0, 53.0, 1600.0, 1024.0)
        );
        // A title strip above the screen's top cannot be grabbed.
        assert_eq!(
            fit_frame((100.0, -200.0, 800.0, 600.0), &[laptop], laptop).1,
            53.0
        );
        // 60 points of a strip on screen is too little to grab.
        assert_eq!(
            fit_frame((1668.0, 100.0, 800.0, 600.0), &[laptop], laptop).0,
            28.0
        );
    }

    #[test]
    fn every_action_has_a_unique_id_a_known_menu_and_a_chord() {
        let file: ActionsFile = serde_json::from_str(ACTIONS_JSON).unwrap();
        let mut ids = std::collections::HashSet::new();
        for a in &file.actions {
            assert!(ids.insert(a.id.as_str()), "{} is listed twice", a.id);
            assert!(
                ["File", "Edit", "Shell", "View", "Window"].contains(&a.menu.as_str()),
                "{}",
                a.menu
            );
            assert!(a.accel.contains("Cmd+"), "{} needs a ⌘ chord (D17)", a.id);
            assert!(!a.label.is_empty());
        }
    }
}
