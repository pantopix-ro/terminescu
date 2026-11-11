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

/// Fires the newest tail timer.
fn tick(plugin: &ClaudePlugin, cx: &Cx) {
    let token = cx.timers.lock().unwrap().last().unwrap().1.clone();
    plugin.on_event(&CoreEvent::Timer { token }, cx);
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
    assert_eq!(got["base"], repo.display().to_string());
    // A tool result tells the UI half to read the list again.
    cx.emits.lock().unwrap().clear();
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "PostToolUse", "session_id": "sess-1", "tool_name": "Bash" }),
    );
    assert!(
        cx.emits
            .lock()
            .unwrap()
            .contains(&("changed".to_string(), json!({ "pane": "p1" })))
    );
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
    assert_eq!(
        cx.timers.lock().unwrap().last().unwrap(),
        &(Duration::from_secs(1), "tail:1".into()),
        "a working session reads every second"
    );

    // A subagent's line does not count; the main session's latest entry does.
    std::fs::write(&transcript, &full).unwrap();
    tick(&plugin, &cx);
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
    tick(&plugin, &cx);
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
    tick(&plugin, &cx);
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

#[test]
fn history_adds_a_late_session_and_reopens_a_resumed_one() {
    let base = folder("history-late");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    // A session the hook first sees mid-way still lands, from any event with a cwd.
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "UserPromptSubmit", "session_id": "sess-late",
                "cwd": "/Users/owner/src/shop-api", "prompt": "Rename the client" }),
    );
    // Without a cwd an unknown session stays out.
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "Stop", "session_id": "sess-nocwd" }),
    );
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionEnd", "session_id": "sess-late" }),
    );
    let list = plugin.call("history", json!({ "row": "s1" }), &cx).unwrap();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["prompt"], "Rename the client");
    assert!(list[0]["ended"].is_u64());

    // `claude --resume` starts it again: open, prompt kept.
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionStart", "session_id": "sess-late", "source": "resume",
                "cwd": "/Users/owner/src/shop-api" }),
    );
    let list = plugin.call("history", json!({ "row": "s1" }), &cx).unwrap();
    assert!(
        list[0]["ended"].is_null(),
        "a resumed session is open again"
    );
    assert_eq!(list[0]["prompt"], "Rename the client");
}

#[test]
fn memory_files_open_from_the_repository_root_and_transcripts_reveal() {
    let base = folder("memory");
    let config = base.join("claude-config");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"))
        .with_config(config.clone());
    let repo = base.join("shop-api");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let cx = Cx::one_pane(repo.join("src").to_str().unwrap());

    let out = plugin
        .call(
            "memory",
            json!({ "row": "s1", "pane": "p1", "which": "project" }),
            &cx,
        )
        .unwrap();
    let project = repo.join("CLAUDE.md").display().to_string();
    assert_eq!(out["path"], project.as_str());
    let out = plugin
        .call("memory", json!({ "row": "s1", "which": "user" }), &cx)
        .unwrap();
    let user = config.join("CLAUDE.md").display().to_string();
    assert_eq!(out["path"], user.as_str());
    assert_eq!(
        cx.opened.lock().unwrap().as_slice(),
        [("s1".to_string(), project), ("s1".to_string(), user)],
        "a missing file still opens, empty, at its path"
    );
    assert!(
        !repo.join("CLAUDE.md").exists(),
        "nothing is written before ⌘S"
    );
    assert!(
        plugin
            .call("memory", json!({ "row": "s1", "which": "team" }), &cx)
            .is_err()
    );

    // Before Claude writes a transcript there is nothing to show.
    assert!(
        plugin
            .call(
                "revealTranscripts",
                json!({ "row": "s1", "pane": "p1" }),
                &cx
            )
            .is_err()
    );
    let folder = config.join("projects/-x");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("sess-77.jsonl"), "{}\n").unwrap();
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "SessionStart", "session_id": "sess-77", "source": "startup",
                "cwd": repo.join("src").display().to_string() }),
    );
    plugin
        .call(
            "revealTranscripts",
            json!({ "row": "s1", "pane": "p1" }),
            &cx,
        )
        .unwrap();
    assert_eq!(
        cx.revealed.lock().unwrap().as_slice(),
        [folder.join("sess-77.jsonl").display().to_string()]
    );
}

#[test]
fn settings_check_their_values_and_reach_the_meter_and_the_prune() {
    let base = folder("settings");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    let got = plugin.call("settings", json!({}), &cx).unwrap();
    assert_eq!(got["meterThreshold"], 80);
    assert_eq!(got["baselineCapMb"], 500);
    assert_eq!(got["baselineDays"], 7);
    assert_eq!(got["defaults"]["window"], 200_000);

    let set = |key: &str, value: Value| {
        plugin.call("setSetting", json!({ "key": key, "value": value }), &cx)
    };
    assert!(set("meterThreshold", json!(0)).is_err());
    assert!(set("baselineCapMb", json!("lots")).is_err());
    assert!(set("contextWindows", json!({ "": 1_000_000 })).is_err());
    assert!(set("contextWindows", json!({ "claude-opus-5": 12 })).is_err());
    assert!(set("color", json!(1)).is_err());
    let got = set("contextWindows", json!({ " claude-opus-5 ": 1_000_000 })).unwrap();
    assert_eq!(got["contextWindows"], json!({ "claude-opus-5": 1_000_000 }));
    let got = set("baselineDays", json!(2)).unwrap();
    assert_eq!(got["baselineDays"], 2);
    assert!(
        cx.timers
            .lock()
            .unwrap()
            .iter()
            .any(|(_, t)| t == "prune-now"),
        "a new limit prunes at once"
    );
    // Null puts the default back.
    let got = set("baselineDays", Value::Null).unwrap();
    assert_eq!(got["baselineDays"], 7);
    assert_eq!(
        terminescu_plugin_claude::settings::limits(&cx),
        (Duration::from_secs(7 * 24 * 3600), 500 * 1024 * 1024)
    );
    set("baselineCapMb", json!(20)).unwrap();
    assert_eq!(
        terminescu_plugin_claude::settings::limits(&cx).1,
        20 * 1024 * 1024
    );

    // The per-file limit goes where the hook reads it (polish-claude-insights D3).
    let store = base.join("support/baselines");
    assert_eq!(got["baselineFileMb"], 5);
    assert!(set("baselineFileMb", json!(101)).is_err());
    let got = set("baselineFileMb", json!(1)).unwrap();
    assert_eq!(got["baselineFileMb"], 1);
    assert_eq!(
        terminescu_plugin_api::baseline::file_cap(&store),
        1024 * 1024
    );
    set("baselineFileMb", Value::Null).unwrap();
    assert!(!store.join("cap").exists());
    cx.settings
        .lock()
        .unwrap()
        .insert("baselineFileMb".into(), json!(2));
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    assert_eq!(
        terminescu_plugin_api::baseline::file_cap(&store),
        2 * 1024 * 1024,
        "launch writes the saved limit"
    );
}

#[test]
fn the_tail_rests_after_a_turn_and_old_timers_do_nothing() {
    let base = folder("rest");
    let transcript = base.join("sess-7.jsonl");
    std::fs::write(&transcript, "").unwrap();
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    let ev = |name: &str| json!({ "hook_event_name": name, "session_id": "sess-7", "transcript_path": transcript });
    hook(&plugin, &cx, ev("SessionStart"));
    let timers = || cx.timers.lock().unwrap().clone();
    assert_eq!(timers(), [(Duration::from_secs(10), "tail:1".to_string())]);
    // A prompt brings the far timer forward; the far one goes stale.
    hook(&plugin, &cx, ev("UserPromptSubmit"));
    assert_eq!(timers()[1], (Duration::from_secs(1), "tail:2".to_string()));
    hook(&plugin, &cx, ev("PostToolUse"));
    assert_eq!(timers().len(), 2, "a near timer stays");
    plugin.on_event(
        &CoreEvent::Timer {
            token: "tail:1".into(),
        },
        &cx,
    );
    assert_eq!(timers().len(), 2, "a stale timer sets no other");
    hook(&plugin, &cx, ev("Stop"));
    tick(&plugin, &cx);
    assert_eq!(
        timers().last().unwrap().0,
        Duration::from_secs(10),
        "a finished turn reads every 10 s"
    );
}

#[test]
fn a_quiet_transcript_or_an_idle_prompt_ends_the_busy_rate() {
    let base = folder("quiet");
    let transcript = base.join("sess-8.jsonl");
    std::fs::write(&transcript, "").unwrap();
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    let ev = |name: &str| json!({ "hook_event_name": name, "session_id": "sess-8", "transcript_path": transcript });
    let last = || cx.timers.lock().unwrap().last().unwrap().0;
    // An interrupt sends no Stop: 30 unchanged reads end the 1 s rate.
    hook(&plugin, &cx, ev("UserPromptSubmit"));
    assert_eq!(last(), Duration::from_secs(1));
    for _ in 0..40 {
        tick(&plugin, &cx);
        if last() == Duration::from_secs(10) {
            break;
        }
    }
    assert_eq!(last(), Duration::from_secs(10), "a quiet session rests");
    // A late tool result after Stop is followed by the idle prompt.
    hook(&plugin, &cx, ev("PostToolUse"));
    assert_eq!(last(), Duration::from_secs(1));
    let mut idle = ev("Notification");
    idle["notification_type"] = json!("idle_prompt");
    hook(&plugin, &cx, idle);
    tick(&plugin, &cx);
    assert_eq!(last(), Duration::from_secs(10), "idle_prompt clears busy");
}

#[test]
fn the_pane_that_spoke_last_leads_its_tab() {
    let base = folder("lead");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane("/Users/owner/src/shop-api");
    let mut second = cx.panes.lock().unwrap()[0].clone();
    second.pane = "p2".into();
    cx.panes.lock().unwrap().push(second);
    let tab = Target::Tab("t1".into());
    let start = |pane: &str, session: &str, model: &str| {
        plugin.on_event(
            &CoreEvent::Hook {
                pane: pane.into(),
                event: json!({ "hook_event_name": "SessionStart", "session_id": session, "model": model }),
            },
            &cx,
        );
    };
    start("p1", "sess-a", "claude-opus-5-5");
    assert_eq!(cx.text(&tab, "model").as_deref(), Some("opus-5-5"));
    start("p2", "sess-b", "claude-haiku-5-5");
    assert_eq!(cx.text(&tab, "model").as_deref(), Some("haiku-5-5"));
    plugin.on_event(
        &CoreEvent::Hook {
            pane: "p2".into(),
            event: json!({ "hook_event_name": "SessionEnd", "session_id": "sess-b" }),
        },
        &cx,
    );
    assert_eq!(
        cx.text(&tab, "model").as_deref(),
        Some("opus-5-5"),
        "the other pane comes back"
    );
    plugin.on_event(&CoreEvent::PaneClosed { pane: "p1".into() }, &cx);
    assert!(cx.text(&tab, "model").is_none());
}
