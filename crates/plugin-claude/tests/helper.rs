//! The Claude Code hook as the plugin's helper (add-plugin-architecture 3.3): what the first-run
//! screen installs and Uninstall Helpers takes out.

use std::path::PathBuf;
use terminescu_plugin_api::{Helper, Plugin};
use terminescu_plugin_claude::{ClaudeHook, ClaudePlugin};

fn settings() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terminescu-helper-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    std::fs::write(&file, "{ \"model\": \"opus\" }\n").unwrap();
    file
}

#[test]
fn the_hook_helper_installs_and_uninstalls_only_its_entry() {
    let hook = ClaudeHook {
        settings: settings(),
        cli: PathBuf::from("/Users/owner/Library/Application Support/Terminescu/bin/terminescu"),
        baselines: None,
    };
    assert!(!hook.installed());
    hook.install().unwrap();
    assert!(hook.installed());
    hook.install().unwrap();
    hook.uninstall().unwrap();
    assert!(!hook.installed());
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hook.settings).unwrap()).unwrap();
    assert_eq!(left["model"], "opus");
}

#[test]
fn the_plugin_offers_the_hook() {
    let p = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &std::env::temp_dir());
    let ids: Vec<_> = p.helpers().iter().map(|h| h.id()).collect();
    assert_eq!(ids, ["hook"]);
}
