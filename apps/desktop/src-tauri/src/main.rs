//! The Terminescu app: hosts the core and one webview per window (D6).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::launcher;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, CommandError, Core, Snapshot, Workspace};
use terminescu_core::views::ViewId;
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

/// Opens a view of a tab's pane. Output goes to `on_output` as raw bytes (D33).
#[tauri::command]
fn pane_open(
    state: State<'_, AppState>,
    tab: String,
    cols: u16,
    rows: u16,
    on_output: Channel<InvokeResponseBody>,
) -> Result<ViewId, CommandError> {
    let sink =
        Arc::new(move |bytes: Vec<u8>| on_output.send(InvokeResponseBody::Raw(bytes)).is_ok());
    state.runtime.open_view(&tab, cols, rows, sink)
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
        .invoke_handler(tauri::generate_handler![
            host_snapshot,
            host_command,
            pane_open,
            pane_close,
            pane_ack,
            pane_resize,
            pane_input,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Terminescu failed to start");
}
