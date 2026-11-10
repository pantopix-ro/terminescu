//! The Claude plugin's insights through its events and calls (add-claude-insights).

mod common;

use common::Cx;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use terminescu_plugin_api::{CoreEvent, Plugin};
use terminescu_plugin_claude::ClaudePlugin;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tsc-insights-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

pub fn hook(plugin: &ClaudePlugin, cx: &Cx, event: Value) {
    plugin.on_event(
        &CoreEvent::Hook {
            pane: "p1".into(),
            event,
        },
        cx,
    );
}

#[test]
fn a_session_in_a_repository_snapshots_and_lists_its_changes() {
    let base = folder("repo");
    let repo = base.join("shop-api");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("retry.ts"), "a\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane(&repo.display().to_string());
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionStart", "session_id": "sess-1", "source": "startup", "cwd": repo }),
    );
    let ready = base.join("support/baselines/sess-1/ready");
    let start = Instant::now();
    while !ready.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the snapshot never finished"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::write(repo.join("retry.ts"), "a\nb\n").unwrap();
    let got = plugin
        .call("changes", json!({ "pane": "p1" }), &cx)
        .unwrap();
    assert_eq!(got["session"], "sess-1");
    assert_eq!(got["tree"], true);
    assert_eq!(
        got["files"][0]["path"],
        repo.join("retry.ts").display().to_string()
    );
    assert_eq!(got["files"][0]["added"], 1);
    let text = plugin
        .call(
            "baseText",
            json!({ "pane": "p1", "path": repo.join("retry.ts") }),
            &cx,
        )
        .unwrap();
    assert_eq!(text["text"], "a\n");

    // A cleared session in the same pane keeps the first baseline.
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionStart", "session_id": "sess-2", "source": "clear", "cwd": repo }),
    );
    let got = plugin
        .call("changes", json!({ "pane": "p1" }), &cx)
        .unwrap();
    assert_eq!(got["session"], "sess-2");
    assert_eq!(got["files"][0]["added"], 1);

    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionEnd", "session_id": "sess-2" }),
    );
    assert!(
        plugin
            .call("changes", json!({ "pane": "p1" }), &cx)
            .is_err(),
        "no session in the pane"
    );
    let meta: Value = serde_json::from_slice(
        &std::fs::read(base.join("support/baselines/sess-2/meta.json")).unwrap(),
    )
    .unwrap();
    assert!(meta["ended"].is_u64());
}

#[test]
fn uninstall_helpers_takes_the_baselines() {
    let base = folder("uninstall");
    std::fs::create_dir_all(base.join("support/baselines/sess-1")).unwrap();
    std::fs::create_dir_all(base.join("claude")).unwrap();
    // SAFETY: this test binary runs the one test that reads CLAUDE_CONFIG_DIR.
    unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", base.join("claude")) };
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let helper = plugin.helpers().pop().unwrap();
    helper.install().unwrap();
    helper.uninstall().unwrap();
    assert!(!base.join("support/baselines").exists());
}
