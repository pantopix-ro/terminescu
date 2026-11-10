//! The Claude plugin's insights through its events and calls (add-claude-insights).

mod common;

use common::Cx;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use terminescu_plugin_api::{CoreEvent, Plugin, Target, Tone};
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

fn fixture(name: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks")).join(name)
}

#[test]
fn a_fixture_transcript_fills_the_meter_chips_and_todos() {
    let base = folder("meter");
    // The transcript grows while the session runs: half now, the rest before the next tick.
    let full = std::fs::read_to_string(fixture("transcript-insights.jsonl")).unwrap();
    let lines: Vec<&str> = full.lines().collect();
    let transcript = base.join("sess-9.jsonl");
    std::fs::write(&transcript, format!("{}\n", lines[..3].join("\n"))).unwrap();
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    let tab = Target::Tab("t1".into());
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "UserPromptSubmit", "session_id": "sess-9",
                "permission_mode": "plan", "transcript_path": transcript }),
    );
    assert_eq!(cx.text(&tab, "context").as_deref(), Some("25%"));
    assert_eq!(cx.text(&tab, "model").as_deref(), Some("opus-5-5"));
    assert_eq!(cx.text(&tab, "mode").as_deref(), Some("plan"));
    let todos = plugin.call("todos", json!({ "pane": "p1" }), &cx).unwrap();
    assert_eq!(todos["todos"].as_array().unwrap().len(), 4);
    assert_eq!(todos["todos"][0]["state"], "inProgress");
    assert!(
        cx.emits
            .lock()
            .unwrap()
            .iter()
            .any(|(e, _)| e == "insights")
    );
    assert_eq!(cx.timers.lock().unwrap().last().unwrap().1, "tail");

    // A subagent's line does not count; the main session's latest entry does.
    std::fs::write(&transcript, &full).unwrap();
    plugin.on_event(
        &CoreEvent::Timer {
            token: "tail".into(),
        },
        &cx,
    );
    let meter = cx.decoration(&tab, "context").unwrap();
    assert_eq!(meter.text, "86%");
    assert_eq!(meter.tone, Tone::Warn);
    assert_eq!(
        meter.tip.as_deref(),
        Some("172k of 200k tokens · claude-opus-5-5")
    );

    // A window set for the model brings it under the threshold.
    cx.settings.lock().unwrap().insert(
        "contextWindows".into(),
        json!({ "claude-opus-5-5": 1_000_000 }),
    );
    plugin.on_event(
        &CoreEvent::Timer {
            token: "tail".into(),
        },
        &cx,
    );
    let meter = cx.decoration(&tab, "context").unwrap();
    assert_eq!((meter.text.as_str(), meter.tone), ("17%", Tone::Neutral));

    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionEnd", "session_id": "sess-9" }),
    );
    assert!(
        cx.decoration(&tab, "context").is_none(),
        "an ended session takes its meter"
    );
    let timers = cx.timers.lock().unwrap().len();
    plugin.on_event(
        &CoreEvent::Timer {
            token: "tail".into(),
        },
        &cx,
    );
    assert_eq!(
        cx.timers.lock().unwrap().len(),
        timers,
        "no session, no tail timer"
    );
}

#[test]
fn history_keeps_50_newest() {
    let base = folder("history");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    for i in 0..52 {
        let id = format!("sess-{i:02}");
        hook(
            &plugin,
            &cx,
            json!({ "hook_event_name": "SessionStart", "session_id": id, "source": "startup",
                    "cwd": format!("/Users/owner/src/shop-api/{i}") }),
        );
        hook(
            &plugin,
            &cx,
            json!({ "hook_event_name": "UserPromptSubmit", "session_id": id,
                    "prompt": format!("\n  Fix the retry for PROJ-{i}\nand add a test") }),
        );
        // A second prompt keeps the first one's line.
        hook(
            &plugin,
            &cx,
            json!({ "hook_event_name": "UserPromptSubmit", "session_id": id, "prompt": "thanks" }),
        );
        hook(
            &plugin,
            &cx,
            json!({ "hook_event_name": "SessionEnd", "session_id": id }),
        );
        // Sessions start a second apart.
        let mut data = cx.row_data.lock().unwrap();
        if let Some(Value::Array(list)) = data.get_mut(&("s1".to_string(), "sessions".to_string()))
        {
            for s in list.iter_mut() {
                if s["id"] == id.as_str() {
                    s["started"] = json!(1000 + i);
                }
            }
        }
    }
    let list = plugin.call("history", json!({ "row": "s1" }), &cx).unwrap();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 50);
    assert_eq!(list[0]["id"], "sess-51");
    assert_eq!(list[49]["id"], "sess-02");
    assert_eq!(list[0]["prompt"], "Fix the retry for PROJ-51");
    assert!(list[0]["ended"].is_u64());

    plugin
        .call("resume", json!({ "row": "s1", "session": "sess-40" }), &cx)
        .unwrap();
    assert_eq!(
        cx.typed.lock().unwrap().as_slice(),
        [(
            "s1".to_string(),
            "/Users/owner/src/shop-api/40".to_string(),
            "claude --resume sess-40".to_string()
        )]
    );
    assert!(
        plugin
            .call("resume", json!({ "row": "s1", "session": "sess-00" }), &cx)
            .is_err(),
        "a session the row no longer keeps"
    );
}
