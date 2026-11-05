//! Tab drags across windows (task 8.1, D20). Inside one window dnd-kit runs the drag. The core
//! tracks the global cursor while it lasts: over another window it tells that window where the
//! cursor is, so the window lights the drop target under it; outside every window a small ghost
//! window follows the cursor. On release the core says where the drop landed: back in the
//! source window, in another window (which resolves the target at that point), or outside all
//! of them, where a new window opens with the tab.

use crate::AppState;
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow};
use terminescu_core::state::{Command, Id};

/// The ghost window's label. It is no workspace window: loops over windows skip it.
pub const GHOST: &str = "drag-ghost";
/// To the window under the cursor: a [`Point`], or null when the cursor left it.
pub const HOVER_EVENT: &str = "host://drag-hover";
/// To the window the tab was dropped on: a [`Point`] to resolve the target at.
pub const DROP_EVENT: &str = "host://drag-drop";
/// To the ghost: the dragged tab's title.
pub const GHOST_EVENT: &str = "host://drag-ghost";
const POLL: Duration = Duration::from_millis(16);
/// A drag whose window never ended it (it reloaded or closed) stops after this.
const MAX_DRAG: Duration = Duration::from_secs(120);
/// The ghost sits this far right of and below the cursor, in logical pixels.
const GHOST_OFFSET: f64 = 14.0;

/// Where a new window opens, in screen pixels with that screen's scale: set by a drop outside
/// every window, read by the patch emitter.
static NEXT_WINDOW_AT: Mutex<Option<(f64, f64, f64)>> = Mutex::new(None);

pub fn take_window_at() -> Option<(f64, f64, f64)> {
    lock(&NEXT_WINDOW_AT).take()
}

/// Locks are held only to read or write this state, never across a window call: those block
/// on the main thread, where sync commands wait for the same lock.
#[derive(Default)]
pub struct Drags {
    current: Mutex<Option<Drag>>,
    /// Bumped by every begin and end; a tracker stops when it changes. A drag's token.
    generation: AtomicU64,
}

#[derive(Clone)]
struct Drag {
    token: u64,
    source: String,
    tab_id: Id,
    title: String,
    /// The other window that last got a hover, to clear when the cursor leaves it.
    hovered: Option<String>,
    /// The last point sent, so a still cursor sends nothing.
    sent: Option<(f64, f64)>,
}

/// A point in a window's own coordinates (CSS pixels from its top left).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    tab_id: Id,
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DragEnd {
    /// Released over the source window: its own drop logic decides.
    Local,
    /// Released over another window, which got a [`DROP_EVENT`].
    Remote,
    /// Released outside every window: a new window holds the tab.
    NewWindow,
    /// No drag was tracked under this token, or the drag was cancelled.
    Cancelled,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The workspace window under `pos` (screen pixels) with the point in its coordinates.
/// Overlapping windows have no z-order here, so `prefer` wins when it is one of them.
fn window_under(
    app: &AppHandle,
    pos: PhysicalPosition<f64>,
    prefer: &str,
) -> Option<(String, f64, f64)> {
    let mut found = None;
    for (label, w) in app.webview_windows() {
        if label == GHOST || !w.is_visible().unwrap_or(false) || w.is_minimized().unwrap_or(false) {
            continue;
        }
        let (Ok(at), Ok(size), Ok(inner), Ok(scale)) = (
            w.outer_position(),
            w.outer_size(),
            w.inner_position(),
            w.scale_factor(),
        ) else {
            continue;
        };
        let inside = pos.x >= at.x as f64
            && pos.y >= at.y as f64
            && pos.x < at.x as f64 + size.width as f64
            && pos.y < at.y as f64 + size.height as f64;
        if !inside {
            continue;
        }
        let local = (
            (pos.x - inner.x as f64) / scale,
            (pos.y - inner.y as f64) / scale,
        );
        if label == prefer {
            return Some((label, local.0, local.1));
        }
        found.get_or_insert((label, local.0, local.1));
    }
    found
}

fn ghost(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window(GHOST) {
        return Some(w);
    }
    // Built on the first drag and kept hidden between drags. It never takes the focus or a click.
    let built =
        tauri::WebviewWindowBuilder::new(app, GHOST, WebviewUrl::App("index.html#ghost".into()))
            .title("")
            .inner_size(220.0, 30.0)
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .visible(false)
            .build();
    match built {
        Ok(w) => {
            let _ = w.set_ignore_cursor_events(true);
            Some(w)
        }
        Err(e) => {
            eprintln!("terminescu: drag ghost not built: {e}");
            None
        }
    }
}

fn hover(app: &AppHandle, label: &str, point: Option<&Point>) {
    if let Err(e) = app.emit_to(label, HOVER_EVENT, point) {
        eprintln!("terminescu: drag hover not sent: {e}");
    }
}

fn hide_ghost(app: &AppHandle) {
    if let Some(g) = app.get_webview_window(GHOST) {
        let _ = g.hide();
    }
}

/// One step of the tracker: light the window under the cursor and move the ghost.
fn track(app: &AppHandle, drags: &Drags, token: u64) {
    let Some(drag) = lock(&drags.current).clone().filter(|d| d.token == token) else {
        return;
    };
    let Ok(pos) = app.cursor_position() else {
        return;
    };
    let under = window_under(app, pos, &drag.source);
    let other = under.as_ref().filter(|(l, ..)| *l != drag.source);
    let mut hovered = drag.hovered.clone();
    let mut sent = drag.sent;
    if hovered.as_deref() != other.map(|(l, ..)| l.as_str())
        && let Some(old) = hovered.take()
    {
        hover(app, &old, None);
        sent = None;
    }
    if let Some((label, x, y)) = other
        && sent != Some((*x, *y))
    {
        let point = Point {
            tab_id: drag.tab_id.clone(),
            x: *x,
            y: *y,
        };
        hover(app, label, Some(&point));
        hovered = Some(label.clone());
        sent = Some((*x, *y));
    }
    if let Some(g) = app.get_webview_window(GHOST) {
        if under.as_ref().is_some_and(|(l, ..)| *l == drag.source) {
            let _ = g.hide();
        } else {
            let scale = g.scale_factor().unwrap_or(1.0);
            let _ = g.set_position(PhysicalPosition::new(
                pos.x + GHOST_OFFSET * scale,
                pos.y + GHOST_OFFSET * scale,
            ));
            if !g.is_visible().unwrap_or(true) {
                let _ = g.show();
            }
        }
    }
    let mut guard = lock(&drags.current);
    if let Some(d) = guard.as_mut().filter(|d| d.token == token) {
        d.hovered = hovered;
        d.sent = sent;
    } else if let Some(h) = hovered {
        // The drag ended while this step ran: take back the hover it just sent.
        drop(guard);
        hover(app, &h, None);
    }
}

/// Ends the drag `token` (any drag when None) without a drop: hides the ghost and clears the
/// hover. Returns it unless the target is `keep`, which keeps its light for the drop.
fn finish(app: &AppHandle, drags: &Drags, token: Option<u64>, keep: Option<&str>) -> Option<Drag> {
    let drag = {
        let mut guard = lock(&drags.current);
        if guard
            .as_ref()
            .is_none_or(|d| token.is_some_and(|t| t != d.token))
        {
            return None;
        }
        drags.generation.fetch_add(1, Ordering::SeqCst);
        guard.take()?
    };
    hide_ghost(app);
    if let Some(h) = drag.hovered.as_deref().filter(|h| Some(*h) != keep) {
        hover(app, h, None);
    }
    Some(drag)
}

/// The window `label` closed: a drag that started there can never end, so it ends now.
pub fn cancel_from(app: &AppHandle, label: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let token = lock(&state.drags.current)
        .as_ref()
        .filter(|d| d.source == label)
        .map(|d| d.token);
    if token.is_some() {
        finish(app, &state.drags, token, None);
    }
}

/// A tab drag started in `window` (dnd-kit passed its activation distance). Returns the
/// drag's token for `drag_end`.
#[tauri::command(async)]
pub fn drag_begin(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    tab_id: Id,
    title: String,
) -> u64 {
    finish(&app, &state.drags, None, None);
    let token = state.drags.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *lock(&state.drags.current) = Some(Drag {
        token,
        source: window.label().to_string(),
        tab_id,
        title: title.clone(),
        hovered: None,
        sent: None,
    });
    if ghost(&app).is_some() {
        let _ = app.emit_to(GHOST, GHOST_EVENT, &title);
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let started = std::time::Instant::now();
        while state.drags.generation.load(Ordering::SeqCst) == token {
            if started.elapsed() > MAX_DRAG {
                finish(&app, &state.drags, Some(token), None);
                break;
            }
            track(&app, &state.drags, token);
            std::thread::sleep(POLL);
        }
    });
    token
}

/// The ghost asks for the title when it loads, since it may miss the first event.
#[tauri::command(async)]
pub fn drag_title(state: State<'_, AppState>) -> Option<String> {
    lock(&state.drags.current).as_ref().map(|d| d.title.clone())
}

/// The drag `token` ended: dropped (`drop`), or cancelled with Esc.
#[tauri::command(async)]
pub fn drag_end(
    app: AppHandle,
    state: State<'_, AppState>,
    token: u64,
    drop: bool,
) -> Result<DragEnd, String> {
    let current = lock(&state.drags.current)
        .clone()
        .filter(|d| d.token == token);
    let Some(drag) = current else {
        return Ok(DragEnd::Cancelled);
    };
    if !drop {
        finish(&app, &state.drags, Some(token), None);
        return Ok(DragEnd::Cancelled);
    }
    let pos = app.cursor_position().map_err(|e| e.to_string())?;
    let under = window_under(&app, pos, &drag.source);
    // The target keeps its light until the drop resolves there.
    let keep = under.as_ref().map(|(l, ..)| l.as_str());
    let Some(drag) = finish(&app, &state.drags, Some(token), keep) else {
        return Ok(DragEnd::Cancelled);
    };
    match under {
        Some((label, ..)) if label == drag.source => Ok(DragEnd::Local),
        Some((label, x, y)) => {
            let point = Point {
                tab_id: drag.tab_id,
                x,
                y,
            };
            app.emit_to(label.as_str(), DROP_EVENT, &point)
                .map_err(|e| e.to_string())?;
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.set_focus();
            }
            Ok(DragEnd::Remote)
        }
        None => {
            // The scale of the screen under the cursor, for the new window's offset.
            let scale = app
                .monitor_from_point(pos.x, pos.y)
                .ok()
                .flatten()
                .map(|m| m.scale_factor())
                .unwrap_or(1.0);
            *lock(&NEXT_WINDOW_AT) = Some((pos.x, pos.y, scale));
            let moved = state.runtime.command(Command::MoveTabToNewWindow {
                tab_id: drag.tab_id,
            });
            if let Err(e) = moved {
                lock(&NEXT_WINDOW_AT).take();
                return Err(e.to_string());
            }
            Ok(DragEnd::NewWindow)
        }
    }
}
