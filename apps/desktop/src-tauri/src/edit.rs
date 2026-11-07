//! The pixel context menu's Edit items (task 8.55): Undo, Redo, Cut, Copy and Paste go down the
//! responder chain as the Edit menu's items do, so the web view handles them as it would a menu
//! pick or a chord: the terminal's copy handler, the editor's undo, a field's paste.

use tauri::AppHandle;

/// The selector for an Edit action, as the predefined Edit menu items send it.
fn selector(action: &str) -> Option<&'static str> {
    Some(match action {
        "undo" => "undo:",
        "redo" => "redo:",
        "cut" => "cut:",
        "copy" => "copy:",
        "paste" => "paste:",
        _ => return None,
    })
}

#[tauri::command]
pub fn edit_command(app: AppHandle, action: String) -> Result<(), String> {
    let name = selector(&action).ok_or_else(|| format!("unknown edit action {action}"))?;
    app.run_on_main_thread(move || send(name))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn send(name: &'static str) {
    use objc2::MainThreadMarker;
    use objc2::runtime::Sel;
    use objc2_app_kit::NSApplication;
    // run_on_main_thread runs this on the main thread.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let sel = Sel::register(&std::ffi::CString::new(name).expect("no NUL in a selector"));
    // SAFETY: the selector is one of the standard Edit actions, and a nil target sends it to
    // the first responder, as a menu item with no target does.
    unsafe {
        NSApplication::sharedApplication(mtm).sendAction_to_from(sel, None, None);
    }
}

#[cfg(not(target_os = "macos"))]
fn send(_: &'static str) {}

#[cfg(test)]
mod tests {
    use super::selector;

    #[test]
    fn edit_actions_map_to_the_standard_selectors() {
        assert_eq!(selector("paste"), Some("paste:"));
        assert_eq!(selector("redo"), Some("redo:"));
        assert_eq!(selector("selectAll"), None);
        assert_eq!(selector("terminate"), None);
    }
}
