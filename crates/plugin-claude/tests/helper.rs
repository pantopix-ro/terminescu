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
        read_only: false,
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

#[test]
fn a_build_on_a_separate_data_folder_leaves_the_settings_alone() {
    let file = settings().with_file_name("dev-settings.json");
    std::fs::write(&file, "{ \"model\": \"opus\" }\n").unwrap();
    let hook = ClaudeHook {
        settings: file,
        cli: PathBuf::from("/Users/owner/.terminescu-dev/bin/terminescu"),
        baselines: None,
        read_only: true,
    };
    hook.install().unwrap();
    assert!(!hook.installed());
    let text = std::fs::read_to_string(&hook.settings).unwrap();
    assert_eq!(text, "{ \"model\": \"opus\" }\n");
}
