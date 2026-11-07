//! Signed updates from the GitHub Releases feed (add-public-release D4). The app checks at
//! launch, every 24 hours and from Terminescu › Check for Updates, downloads a newer release,
//! verifies its minisign signature, and installs it only when the person picks Restart to
//! update. The shells live in the daemon (D32), so the new app reattaches to them.

use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};
use terminescu_protocol::public;

/// The UI hears this with the new version once an update is downloaded and verified.
pub const UPDATE_EVENT: &str = "host://update";

/// The updater's public key. The release job builds it in from the repository variable of the
/// same name; a build without it never checks (docs/releasing.md).
const PUBKEY: Option<&str> = option_env!("TERMINESCU_UPDATER_PUBKEY");
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// A downloaded, verified update waiting for the person's yes.
#[derive(Default)]
pub struct Pending(Mutex<Option<(Update, Vec<u8>)>>);

/// Checks now and then once a day, in the background.
pub fn start(app: &AppHandle) {
    if PUBKEY.is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match fetch(&app).await {
                Ok(Some(version)) => {
                    let _ = app.emit(UPDATE_EVENT, version);
                }
                Ok(None) => {}
                Err(e) => eprintln!("terminescu: update check failed: {e}"),
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// Downloads and verifies a newer release, if there is one, and keeps it for `update_restart`.
async fn fetch(app: &AppHandle) -> Result<Option<String>, String> {
    let pending = app.state::<Pending>();
    if let Some((update, _)) = pending.0.lock().map_err(|e| e.to_string())?.as_ref() {
        return Ok(Some(update.version.clone()));
    }
    let key = PUBKEY.ok_or("Updates are off in this build: it has no update key.")?;
    let feed = Url::parse(&public::update_feed()).map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![feed])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    // `download` checks the signature against PUBKEY and fails on a mismatch.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| format!("update {} refused: {e}", update.version))?;
    let version = update.version.clone();
    *pending.0.lock().map_err(|e| e.to_string())? = Some((update, bytes));
    Ok(Some(version))
}

/// Terminescu › Check for Updates: the newer version ready to install, or none.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<String>, String> {
    fetch(&app)
        .await
        .inspect_err(|e| eprintln!("terminescu: update check failed: {e}"))
}

/// Restart to update: installs the verified download and restarts into it.
#[tauri::command]
pub fn update_restart(app: AppHandle) -> Result<(), String> {
    let taken = app
        .state::<Pending>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take();
    let (update, bytes) = taken.ok_or("No update is ready.")?;
    update
        .install(bytes)
        .map_err(|e| format!("update {} not installed: {e}", update.version))?;
    app.restart()
}

#[cfg(test)]
mod tests {
    /// The plugin reads `plugins.updater` at launch and refuses to start without it. The key
    /// and feed stay empty there: `fetch` sets both on its own updater.
    #[test]
    fn tauri_conf_has_an_updater_block_the_plugin_accepts() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let block = conf["plugins"]["updater"].clone();
        let parsed: tauri_plugin_updater::Config = serde_json::from_value(block).unwrap();
        assert!(parsed.pubkey.is_empty() && parsed.endpoints.is_empty());
    }
}
