//! The Dock icon's right-click menu: every window with its sidebar rows (add-dock-menu).
//!
//! Each window reports its rows with the names its sidebar shows (D3). When macOS asks for the
//! Dock menu, the app builds it from those rows in workspace order (D4); a pick focuses the
//! window and, for a row, tells that window to select it (D6).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

/// Sent to a window when the Dock menu picks one of its rows; the payload is the side tab id.
pub(crate) const REVEAL_SIDE_EVENT: &str = "host://reveal-side";

/// One sidebar row as the window's sidebar names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Row {
    pub id: String,
    pub name: String,
    pub active: bool,
}

/// The rows each window reported last, by window label.
#[derive(Default)]
pub(crate) struct Rows(Mutex<HashMap<String, Vec<Row>>>);

impl Rows {
    pub(crate) fn forget(&self, window: &str) {
        crate::lock(&self.0).remove(window);
    }
}

/// A window reports its rows when they, their names or the shown row change.
#[tauri::command]
pub(crate) fn dock_rows(window: WebviewWindow, rows: Vec<Row>, state: State<'_, Rows>) {
    crate::lock(&state.0).insert(window.label().to_owned(), rows);
}

/// One line of the Dock menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    Window {
        id: String,
        title: String,
    },
    Row {
        id: String,
        title: String,
        active: bool,
    },
    Separator,
}

/// The menu for `order` (window labels in workspace order) from the reported rows. A window that
/// has not reported yet shows its header only.
pub(crate) fn entries(order: &[String], rows: &HashMap<String, Vec<Row>>) -> Vec<Entry> {
    let mut out = Vec::new();
    for (n, window) in order.iter().enumerate() {
        if n > 0 {
            out.push(Entry::Separator);
        }
        let list = rows.get(window).map(Vec::as_slice).unwrap_or_default();
        let title = match list.iter().find(|r| r.active) {
            Some(r) => format!("Window {} · {}", n + 1, r.name),
            None => format!("Window {}", n + 1),
        };
        out.push(Entry::Window {
            id: format!("dock:w:{window}"),
            title,
        });
        out.extend(list.iter().map(|r| Entry::Row {
            id: format!("dock:s:{window}:{}", r.id),
            title: format!("    {}", r.name),
            active: r.active,
        }));
    }
    out
}

/// What a Dock menu id picks: a window, or a row in a window.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pick<'a> {
    Window(&'a str),
    Row(&'a str, &'a str),
}

pub(crate) fn parse(id: &str) -> Option<Pick<'_>> {
    if let Some(w) = id.strip_prefix("dock:w:") {
        return (!w.is_empty()).then_some(Pick::Window(w));
    }
    let (w, s) = id.strip_prefix("dock:s:")?.split_once(':')?;
    (!w.is_empty() && !s.is_empty()).then_some(Pick::Row(w, s))
}

/// Handles a Dock menu pick. Returns false when `id` is not a Dock menu id. A window that is
/// gone makes the pick do nothing.
pub(crate) fn pick(app: &AppHandle, id: &str) -> bool {
    if !id.starts_with("dock:") {
        return false;
    }
    let Some(p) = parse(id) else { return true };
    let (window, side) = match p {
        Pick::Window(w) => (w, None),
        Pick::Row(w, s) => (w, Some(s)),
    };
    let Some(w) = app.get_webview_window(window) else {
        return true;
    };
    let _ = w.unminimize();
    let _ = w.set_focus();
    if let Some(side) = side
        && let Err(e) = app.emit_to(window, REVEAL_SIDE_EVENT, side)
    {
        eprintln!("terminescu: dock pick not sent: {e}");
    }
    true
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Entry, Rows, entries};
    use muda::ContextMenu;
    use std::cell::RefCell;
    use std::sync::OnceLock;
    use tauri::{AppHandle, Manager};

    static APP: OnceLock<AppHandle> = OnceLock::new();

    thread_local! {
        /// The last menu handed to macOS; it lives until the next request (D4).
        static MENU: RefCell<Option<muda::Menu>> = const { RefCell::new(None) };
    }

    fn build(app: &AppHandle) -> Option<muda::Menu> {
        let state = app.try_state::<crate::AppState>()?;
        let rows = app.try_state::<Rows>()?;
        let order: Vec<String> = state
            .runtime
            .snapshot()
            .workspace
            .windows
            .iter()
            .map(|w| w.id.clone())
            .filter(|l| app.get_webview_window(l).is_some())
            .collect();
        let list = entries(&order, &crate::lock(&rows.0));
        let menu = muda::Menu::new();
        for e in list {
            let added = match e {
                Entry::Window { id, title } => {
                    menu.append(&muda::MenuItem::with_id(id, title, true, None))
                }
                Entry::Row { id, title, active } => {
                    menu.append(&muda::CheckMenuItem::with_id(id, title, true, active, None))
                }
                Entry::Separator => menu.append(&muda::PredefinedMenuItem::separator()),
            };
            if let Err(e) = added {
                eprintln!("terminescu: dock menu item not added: {e}");
            }
        }
        Some(menu)
    }

    /// `-[delegate applicationDockMenu:]`: macOS calls it on the main thread at each right click.
    extern "C-unwind" fn dock_menu(
        _this: *mut objc2::runtime::AnyObject,
        _cmd: objc2::runtime::Sel,
        _sender: *mut objc2::runtime::AnyObject,
    ) -> *mut std::ffi::c_void {
        let Some(app) = APP.get() else {
            return std::ptr::null_mut();
        };
        MENU.with(|m| {
            let mut m = m.borrow_mut();
            *m = build(app);
            m.as_ref()
                .map_or(std::ptr::null_mut(), |menu| menu.ns_menu())
        })
    }

    /// Adds `applicationDockMenu:` to the app delegate's class (D2). Runs on the main thread.
    pub(crate) fn install(app: &AppHandle) {
        use objc2::MainThreadMarker;
        use objc2::runtime::{AnyObject, Sel};
        use objc2_app_kit::NSApplication;
        let _ = APP.set(app.clone());
        let Some(mtm) = MainThreadMarker::new() else {
            eprintln!("terminescu: dock menu not installed off the main thread");
            return;
        };
        let ns_app = NSApplication::sharedApplication(mtm);
        let Some(delegate) = ns_app.delegate() else {
            eprintln!("terminescu: dock menu not installed: no app delegate");
            return;
        };
        let obj: *const AnyObject = (&*delegate as *const _ as *const AnyObject).cast();
        // SAFETY: the delegate is a live object; the function has the C signature of
        // `- (NSMenu *)applicationDockMenu:(NSApplication *)sender`, type string "@@:@".
        let added = unsafe {
            let cls = objc2::ffi::object_getClass(obj) as *mut objc2::runtime::AnyClass;
            let imp: objc2::runtime::Imp = std::mem::transmute(
                dock_menu
                    as extern "C-unwind" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                    ) -> *mut std::ffi::c_void,
            );
            objc2::ffi::class_addMethod(
                cls,
                objc2::sel!(applicationDockMenu:),
                imp,
                c"@@:@".as_ptr(),
            )
        };
        if !added.as_bool() {
            eprintln!("terminescu: dock menu not installed: the delegate already answers it");
            return;
        }
        // AppKit reads which methods the delegate has when it is set: set it again.
        ns_app.setDelegate(None);
        ns_app.setDelegate(Some(&delegate));
    }
}

/// Puts the windows and rows into the Dock icon's right-click menu.
pub(crate) fn install(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    mac::install(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, name: &str, active: bool) -> Row {
        Row {
            id: id.into(),
            name: name.into(),
            active,
        }
    }

    #[test]
    fn two_windows_list_their_rows_with_the_shown_row_checked() {
        let order = vec!["w1".to_string(), "w2".to_string()];
        let rows = HashMap::from([
            (
                "w1".to_string(),
                vec![row("a", "api", false), row("b", "web", true)],
            ),
            ("w2".to_string(), vec![row("c", "notes", true)]),
        ]);
        assert_eq!(
            entries(&order, &rows),
            vec![
                Entry::Window {
                    id: "dock:w:w1".into(),
                    title: "Window 1 · web".into()
                },
                Entry::Row {
                    id: "dock:s:w1:a".into(),
                    title: "    api".into(),
                    active: false
                },
                Entry::Row {
                    id: "dock:s:w1:b".into(),
                    title: "    web".into(),
                    active: true
                },
                Entry::Separator,
                Entry::Window {
                    id: "dock:w:w2".into(),
                    title: "Window 2 · notes".into()
                },
                Entry::Row {
                    id: "dock:s:w2:c".into(),
                    title: "    notes".into(),
                    active: true
                },
            ]
        );
    }

    #[test]
    fn a_window_that_has_not_reported_shows_its_header_and_gone_windows_drop() {
        let order = vec!["w2".to_string()];
        let rows = HashMap::from([("w1".to_string(), vec![row("a", "api", true)])]);
        assert_eq!(
            entries(&order, &rows),
            vec![Entry::Window {
                id: "dock:w:w2".into(),
                title: "Window 1".into()
            }]
        );
    }

    #[test]
    fn dock_ids_parse_and_others_do_not() {
        assert_eq!(parse("dock:w:w1"), Some(Pick::Window("w1")));
        assert_eq!(parse("dock:s:w1:s-9"), Some(Pick::Row("w1", "s-9")));
        assert_eq!(parse("dock:s:w1"), None);
        assert_eq!(parse("dock:w:"), None);
        assert_eq!(parse("new_tab"), None);
    }
}
