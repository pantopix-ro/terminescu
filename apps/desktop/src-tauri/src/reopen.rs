//! Undo close (task 8.87): ⇧⌘T reopens the last closed tab or row. The list lives for this
//! session only; a closed row's session and secret variables wait in the daemon until its
//! entry leaves the list or the app quits.

use crate::{AppState, lock, merged};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use terminescu_core::closed::{self, Closed, ClosedList, Entry};
use terminescu_core::state::{Command, Id};

static CLOSED: Mutex<ClosedList> = Mutex::new(ClosedList::new());

/// A closed row whose daemon variables must stay.
pub fn is_parked(side_tab_id: &str) -> bool {
    lock(&CLOSED).is_parked(side_tab_id)
}

fn drop_vars(state: &AppState, entry: &Entry) {
    if let (Some(side), Some(d)) = (
        entry.parked_side_tab(),
        state.runtime.daemon().filter(|d| d.side_vars),
    ) && let Err(e) = d.side_vars_drop(side)
    {
        eprintln!("terminescu: variables of a closed side tab not dropped: {e}");
    }
}

/// Before `cmd` runs: notes the tab or row it closes, with its context and cwds. True when
/// something was noted, for [`forget`] should the command fail.
pub fn note(state: &AppState, cmd: &Command) -> bool {
    let ws = state.runtime.workspace();
    let found = closed::closing(&lock(&state.db), &ws, cmd);
    let closed = match found {
        Ok(Some(c)) => c,
        Ok(None) => return false,
        Err(e) => {
            eprintln!("terminescu: closed tab not kept for reopening: {e}");
            return false;
        }
    };
    let tabs = match &closed {
        Closed::Row { side_tab, .. } => side_tab.tabs.iter().collect::<Vec<_>>(),
        Closed::Tab { tab, .. } => vec![tab],
    };
    let cwds: HashMap<Id, String> = tabs
        .iter()
        .flat_map(|t| t.layout.terminals())
        .filter_map(|p| state.runtime.live_cwd(p).map(|c| (p.clone(), c)))
        .collect();
    let out = lock(&CLOSED).push(Entry { closed, cwds });
    if let Some(old) = out {
        drop_vars(state, &old);
    }
    true
}

/// The close failed: its entry goes again.
pub fn forget() {
    lock(&CLOSED).pop();
}

/// The app quits: the list ends, and its rows' daemon variables with it.
pub fn end(state: &AppState) {
    let all = lock(&CLOSED).drain();
    for e in &all {
        drop_vars(state, e);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reopened {
    side_tab_id: Id,
    tab_id: Id,
}

/// Reopens the last closed tab or row: back to its window when it still has it, else to
/// `window_id`. None when nothing was closed.
#[tauri::command(async)]
pub fn reopen_closed(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: Id,
) -> Result<Option<Reopened>, String> {
    let Some(entry) = lock(&CLOSED).pop() else {
        return Ok(None);
    };
    let cmd = entry.reopen(&state.runtime.workspace(), &window_id);
    if let Err(e) = state.runtime.command_in(cmd, &entry.cwds) {
        drop_vars(&state, &entry);
        return Err(e.to_string());
    }
    match &entry.closed {
        Closed::Row {
            side_tab, context, ..
        } => {
            if let Err(e) = context.restore(&lock(&state.db)) {
                eprintln!("terminescu: context of a reopened side tab not restored: {e}");
            }
            merged(&app, &state, &side_tab.id);
            Ok(Some(Reopened {
                side_tab_id: side_tab.id.clone(),
                tab_id: side_tab.tabs[0].id.clone(),
            }))
        }
        Closed::Tab { tab, .. } => {
            let ws = state.runtime.workspace();
            let (side, _) = ws.locate_tab(&tab.id).ok_or("the tab did not come back")?;
            Ok(Some(Reopened {
                side_tab_id: side.id.clone(),
                tab_id: tab.id.clone(),
            }))
        }
    }
}
