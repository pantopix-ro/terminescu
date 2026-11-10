//! Undo close (task 8.87): ⇧⌘T reopens the last closed tab or row. The list lives for this
//! session only; a closed row's session and secret variables wait in the daemon until its
//! entry leaves the list or the app quits.

use crate::{AppState, lock, merged};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use terminescu_core::closed::{self, Closed, Entry};
use terminescu_core::state::{Command, Id};

fn drop_vars(state: &AppState, entry: &Entry) {
    if let (Some(side), Some(d)) = (
        entry.parked_side_tab(),
        state.runtime.daemon().filter(|d| d.side_vars),
    ) && let Err(e) = d.side_vars_drop(side)
    {
        eprintln!("terminescu: variables of a closed side tab not dropped: {e}");
    }
}

/// Before `cmd` runs: notes the tab or row it closes, with its context and cwds, so the close
/// leaves its daemon variables alone. Returns the close's token for [`done`].
pub fn note(state: &AppState, cmd: &Command) -> Option<u64> {
    if !closed::may_close(cmd) {
        return None;
    }
    let ws = state.runtime.workspace();
    let closed = match closed::closing(&lock(&state.db), &ws, cmd) {
        Ok(c) => c?,
        Err(e) => {
            eprintln!("terminescu: closed tab not kept for reopening: {e}");
            return None;
        }
    };
    let panes: Vec<Id> = match &closed {
        Closed::Row { side_tab, .. } => side_tab.tabs.iter().collect::<Vec<_>>(),
        Closed::Tab { tab, .. } => vec![tab],
    }
    .into_iter()
    .flat_map(|t| t.layout.terminals())
    .cloned()
    .collect();
    let cwds = state.runtime.live_cwds(&panes);
    Some(lock(&state.closed).push(Entry { closed, cwds }))
}

/// After the close ran: a done one can be reopened, a failed one is forgotten, and the oldest
/// entries leave with their rows' daemon variables.
pub fn done(state: &AppState, token: u64, ok: bool) {
    let gone = lock(&state.closed).settle(token, ok);
    for old in &gone {
        drop_vars(state, old);
    }
}

/// The app quits: the list ends, and its rows' daemon variables with it.
pub fn end(state: &AppState) {
    let all = lock(&state.closed).drain();
    for e in &all {
        drop_vars(state, e);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reopened {
    window_id: Id,
    side_tab_id: Id,
    tab_id: Id,
    /// A whole row came back, not one tab.
    row: bool,
}

/// Reopens the last closed tab or row: back to its window when it still has it, else to
/// `window_id`. None when nothing was closed.
#[tauri::command(async)]
pub fn reopen_closed(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: Id,
) -> Result<Option<Reopened>, String> {
    let Some(entry) = lock(&state.closed).pop() else {
        return Ok(None);
    };
    let cmd = entry.reopen(&state.runtime.workspace(), &window_id);
    let target = match &cmd {
        Command::ReopenSideTab { window_id, .. } | Command::ReopenTab { window_id, .. } => {
            window_id.clone()
        }
        _ => window_id.clone(),
    };
    // A reopen that fails keeps its entry, so ⇧⌘T can try again (harden-plugin-host D3).
    if let Err(e) = state.runtime.command_in(cmd, &entry.cwds) {
        let dropped = lock(&state.closed).put_back(entry);
        if let Some(gone) = dropped {
            drop_vars(&state, &gone);
            return Err(format!("Reopen: {e}. It is no longer kept."));
        }
        return Err(format!("Reopen: {e}"));
    }
    let (side_tab_id, tab_id) = match &entry.closed {
        Closed::Row {
            side_tab, context, ..
        } => {
            if let Err(e) = context.restore(&lock(&state.db)) {
                eprintln!("terminescu: context of a reopened side tab not restored: {e}");
            }
            merged(&app, &state, &side_tab.id);
            (side_tab.id.clone(), side_tab.tabs[0].id.clone())
        }
        Closed::Tab { tab, .. } => {
            let ws = state.runtime.workspace();
            let (side, _) = ws.locate_tab(&tab.id).ok_or("the tab did not come back")?;
            (side.id.clone(), tab.id.clone())
        }
    };
    // Back in another window than the one pressed: that window comes forward and shows it.
    if target != window_id {
        if let Some(w) = app.get_webview_window(&target) {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        let pane = state
            .runtime
            .workspace()
            .tab(&tab_id)
            .and_then(|t| t.layout.terminals().first().cloned().cloned());
        if let Some(pane) = pane
            && let Err(e) = app.emit_to(target.as_str(), crate::REVEAL_EVENT, &pane)
        {
            eprintln!("terminescu: reopened tab not shown: {e}");
        }
    }
    Ok(Some(Reopened {
        window_id: target,
        side_tab_id,
        tab_id,
        row: matches!(entry.closed, Closed::Row { .. }),
    }))
}
