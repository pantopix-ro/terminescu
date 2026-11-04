//! The Terminescu app: hosts the core and one webview per window (D6).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use terminescu_core::db::Db;
use terminescu_core::state::{Command, CommandError, Core, Snapshot, Workspace};

/// Event that carries every patch batch to every window.
const PATCHES_EVENT: &str = "host://patches";

struct AppState {
    core: Mutex<Core>,
    _db: Mutex<Db>,
}

#[tauri::command]
fn host_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.core.lock().expect("core lock").snapshot()
}

#[tauri::command]
fn host_command(app: AppHandle, state: State<'_, AppState>, command: Command) -> Result<u64, CommandError> {
    let mut core = state.core.lock().expect("core lock");
    let batch = core.handle(command)?;
    // Emit under the lock so every window receives batches in rev order.
    if let Err(e) = app.emit(PATCHES_EVENT, &batch) {
        eprintln!("terminescu: patch broadcast failed: {e}");
    }
    Ok(batch.rev)
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(dir.join("terminescu.sqlite"))?;
            db.local_owner()?;
            // Layout persistence arrives with task 3.3; until then every start is a first start.
            let mut core = Core::new(Workspace::default());
            let window_id = core.first_start();
            app.manage(AppState { core: Mutex::new(core), _db: Mutex::new(db) });
            open_window(app.handle(), &window_id)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![host_snapshot, host_command])
        .run(tauri::generate_context!())
        .expect("Terminescu failed to start");
}
