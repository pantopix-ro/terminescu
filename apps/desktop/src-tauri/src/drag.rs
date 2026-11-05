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

/// Where a new window opens: set by a drop outside every window, read by the patch emitter.
static NEXT_WINDOW_AT: Mutex<Option<(f64, f64)>> = Mutex::new(None);

pub fn take_window_at() -> Option<(f64, f64)> {
    NEXT_WINDOW_AT.lock().map(|mut g| g.take()).unwrap_or(None)
}

#[derive(Default)]
pub struct Drags {
    current: Mutex<Option<Drag>>,
    /// Bumped by every begin and end; a tracker thread stops when it changes.
    generation: AtomicU64,
}

struct Drag {
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
    /// No drag was tracked, or the drag was cancelled.
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

/// One step of the tracker: light the window under the cursor and move the ghost.
fn track(app: &AppHandle, drags: &Drags) {
    let Ok(pos) = app.cursor_position() else {
        return;
    };
    let mut guard = lock(&drags.current);
    let Some(drag) = guard.as_mut() else {
        return;
    };
    let under = window_under(app, pos, &drag.source);
    let other = under.as_ref().filter(|(l, ..)| *l != drag.source);
    if drag.hovered.as_deref() != other.map(|(l, ..)| l.as_str())
        && let Some(old) = drag.hovered.take()
    {
        hover(app, &old, None);
        drag.sent = None;
    }
    if let Some((label, x, y)) = other
        && drag.sent != Some((*x, *y))
    {
        let point = Point {
            tab_id: drag.tab_id.clone(),
            x: *x,
            y: *y,
        };
        hover(app, label, Some(&point));
        drag.hovered = Some(label.clone());
        drag.sent = Some((*x, *y));
    }
    let Some(g) = app.get_webview_window(GHOST) else {
        return;
    };
    if under.as_ref().is_some_and(|(l, ..)| *l == drag.source) {
        let _ = g.hide();
    } else {
        let scale = g.scale_factor().unwrap_or(1.0);
        let at = PhysicalPosition::new(pos.x + GHOST_OFFSET * scale, pos.y + GHOST_OFFSET * scale);
        let _ = g.set_position(at);
        if !g.is_visible().unwrap_or(true) {
            let _ = g.show();
        }
    }
}

/// A tab drag started in `window` (dnd-kit passed its activation distance). Sync, so it runs
/// on the main thread before the `drag_end` that follows it.
#[tauri::command]
pub fn drag_begin(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    tab_id: Id,
    title: String,
) {
    let drags = &state.drags;
    let generation = drags.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(old) = lock(&drags.current).replace(Drag {
        source: window.label().to_string(),
        tab_id,
        title: title.clone(),
        hovered: None,
        sent: None,
    }) && let Some(h) = old.hovered
    {
        hover(&app, &h, None);
    }
    if ghost(&app).is_some() {
        let _ = app.emit_to(GHOST, GHOST_EVENT, &title);
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let started = std::time::Instant::now();
        while state.drags.generation.load(Ordering::SeqCst) == generation {
            if started.elapsed() > MAX_DRAG {
                // Ends this drag only: a newer one moved the generation on.
                if state
                    .drags
                    .generation
                    .compare_exchange(
                        generation,
                        generation + 1,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_ok()
                    && let Some(d) = lock(&state.drags.current).take()
                {
                    if let Some(h) = d.hovered {
                        hover(&app, &h, None);
                    }
                    if let Some(g) = app.get_webview_window(GHOST) {
                        let _ = g.hide();
                    }
                }
                break;
            }
            track(&app, &state.drags);
            std::thread::sleep(POLL);
        }
    });
}

/// The ghost asks for the title when it loads, since it may miss the first event.
#[tauri::command]
pub fn drag_title(state: State<'_, AppState>) -> Option<String> {
    lock(&state.drags.current).as_ref().map(|d| d.title.clone())
}

/// The drag ended: dropped (`drop`), or cancelled with Esc. Async, so a new window can be
/// built while the main thread runs.
#[tauri::command(async)]
pub fn drag_end(app: AppHandle, state: State<'_, AppState>, drop: bool) -> Result<DragEnd, String> {
    let drags = &state.drags;
    drags.generation.fetch_add(1, Ordering::SeqCst);
    let Some(drag) = lock(&drags.current).take() else {
        return Ok(DragEnd::Cancelled);
    };
    if let Some(g) = app.get_webview_window(GHOST) {
        let _ = g.hide();
    }
    if let Some(h) = &drag.hovered {
        hover(&app, h, None);
    }
    if !drop {
        return Ok(DragEnd::Cancelled);
    }
    let pos = app.cursor_position().map_err(|e| e.to_string())?;
    match window_under(&app, pos, &drag.source) {
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
            let scale = app
                .get_webview_window(&drag.source)
                .and_then(|w| w.scale_factor().ok())
                .unwrap_or(1.0);
            *lock(&NEXT_WINDOW_AT) = Some((pos.x / scale, pos.y / scale));
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
