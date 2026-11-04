//! The Terminescu app: hosts the core and one webview per window (D6).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::launcher;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, CommandError, Core, Snapshot, Workspace};
use terminescu_protocol::paths;

/// Event that carries every patch batch to every window.
const PATCHES_EVENT: &str = "host://patches";

struct AppState {
    runtime: Runtime,
    _db: Mutex<Db>,
}

#[tauri::command]
fn host_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.runtime.snapshot()
}

#[tauri::command]
fn host_command(state: State<'_, AppState>, command: Command) -> Result<u64, CommandError> {
    // The runtime emits each batch under its core lock, so windows see them in rev order.
    state.runtime.command(command)
}

/// The sidecars sit next to the app binary: in `Contents/MacOS` when bundled, in
/// `target/debug` under `tauri dev`.
fn sidecar(name: &str) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent().unwrap_or(Path::new(".")).join(name))
}

/// Installs the daemon and CLI copies (D32, D36) and starts the runtime on the daemon.
fn start_runtime(app: &AppHandle) -> Result<(Runtime, String), Box<dyn std::error::Error>> {
    let bin = paths::bin_dir();
    let daemon = launcher::install(&sidecar("terminescu-ptyd")?, &bin)?;
    if let Err(e) = launcher::install_cli(&sidecar("terminescu")?, &bin) {
        eprintln!("terminescu: CLI copy failed: {e}");
    }
    let run = paths::run_paths();
    paths::ensure_private_dir(&run.dir)?;
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
    // Layout persistence arrives with task 3.3; until then a start adopts the daemon's panes.
    Ok(Runtime::start(
        Core::new(Workspace::default()),
        emit,
        connect,
    )?)
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
            let (runtime, window_id) = start_runtime(app.handle())?;
            app.manage(AppState {
                runtime,
                _db: Mutex::new(db),
            });
            open_window(app.handle(), &window_id)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![host_snapshot, host_command])
        .run(tauri::generate_context!())
        .expect("Terminescu failed to start");
}
