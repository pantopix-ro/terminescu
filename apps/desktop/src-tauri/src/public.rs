//! The public release's app side (add-public-release): About's facts, the feedback routes and
//! the uninstall of the helpers. The menus that reach them are built in `main.rs`.

use crate::crashlog;
use tauri::{AppHandle, Manager};
use terminescu_core::feedback::{self, AppInfo, Route};
use terminescu_core::uninstall;
use terminescu_protocol::paths;

/// The menu items this module answers, in the app menu and Help. They have no chords, so they
/// stay out of actions.json and Settings › Keys.
pub const ABOUT: &str = "about";
pub const CHECK_UPDATES: &str = "check_updates";
pub const FEEDBACK_EMAIL: &str = "feedback_email";
pub const FEEDBACK_ISSUE: &str = "feedback_issue";
pub const PRIVACY: &str = "privacy";
pub const LICENSES: &str = "licenses";
pub const UNINSTALL: &str = "uninstall";

pub fn info(app: &AppHandle) -> AppInfo {
    let macos = std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    AppInfo {
        version: app.package_info().version.to_string(),
        // The release job sets TERMINESCU_BUILD to the commit; a local build says `dev`.
        build: option_env!("TERMINESCU_BUILD").unwrap_or("dev").into(),
        macos,
    }
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    info(&app)
}

/// Opens the draft email or the new issue, and with `include_log` shows app.log in Finder for
/// the person to attach: neither `mailto:` nor a new-issue link can carry a file.
#[tauri::command]
pub fn feedback_open(app: AppHandle, route: Route, include_log: bool) -> Result<(), String> {
    let url = feedback::link(route, &info(&app), include_log);
    open(&[&url])?;
    if include_log {
        let log = crashlog::log_path();
        if log.is_file() {
            open(&["-R", &log.to_string_lossy()])?;
        }
    }
    Ok(())
}

fn open(args: &[&str]) -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .args(args)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Removes the helpers in D9's order: the plugins' helpers (the Claude Code hook), the daemon and its shells (the person agreed
/// in the dialog), the copied binaries and sockets, then the data when asked. The app quits
/// with `app_quit` after the last note.
#[tauri::command]
pub fn uninstall_helpers(app: AppHandle, delete_data: bool) -> Result<(), String> {
    // Every plugin's helpers, on or off, so no tool keeps calling a missing program.
    for h in crate::plugins::helpers() {
        h.uninstall().map_err(|e| e.to_string())?;
    }
    // Stop reconnecting first: after the kill below, a reconnect would copy the daemon back
    // into bin/ and start it again, and it would outlive the uninstall.
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.runtime.stop();
    }
    let bin = paths::bin_dir();
    // The daemon runs from its copy in bin/ (D32): ending it closes the open shells.
    let _ = std::process::Command::new("/usr/bin/pkill")
        .args(["-U", &paths::uid().to_string(), "-f"])
        .arg(format!("{}/ptyd-", bin.display()))
        .status();
    let mut data = Vec::new();
    if let Ok(d) = app.path().app_data_dir() {
        data.push(d);
    }
    if let Ok(d) = app.path().app_cache_dir() {
        data.push(d);
    }
    if let Ok(home) = app.path().home_dir() {
        data.push(home.join("Library/WebKit").join(&app.config().identifier));
    }
    let places = uninstall::Places {
        support: paths::support_dir(),
        run: paths::run_paths().dir,
        data,
    };
    uninstall::helpers(&places, delete_data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_quit(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The menu items above, which have no chord and so stay out of actions.json.
    const MENU_ONLY: [&str; 7] = [
        ABOUT,
        CHECK_UPDATES,
        FEEDBACK_EMAIL,
        FEEDBACK_ISSUE,
        PRIVACY,
        LICENSES,
        UNINSTALL,
    ];

    #[test]
    fn menu_only_ids_never_clash_with_an_action() {
        let actions: serde_json::Value = serde_json::from_str(crate::ACTIONS_JSON).unwrap();
        let ids: Vec<&str> = actions["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["id"].as_str())
            .collect();
        assert!(ids.len() > 20, "actions.json lists the app actions");
        for id in MENU_ONLY {
            assert!(!ids.contains(&id), "{id} is a menu-only item and an action");
        }
    }
}
