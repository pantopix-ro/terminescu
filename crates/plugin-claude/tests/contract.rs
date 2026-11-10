//! The plugin's answers as JSON fixtures (polish-claude-insights D8). The e2e fake and the UI
//! tests read `packages/plugin-claude/contract/*.json`, so they cannot drift from what Rust
//! answers. `TERMINESCU_BLESS=1 cargo test -p terminescu-plugin-claude --test contract`
//! writes them again after a deliberate change.

mod common;

use common::Cx;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use terminescu_plugin_api::{CoreEvent, Plugin};
use terminescu_plugin_claude::{ClaudePlugin, settings};

/// Where the fixtures' repository seems to live.
const SHOWN_ROOT: &str = "/Users/owner/src/shop-api";

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tsc-contract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

/// Compares `value` with the committed fixture, or writes it when blessing.
fn check(name: &str, value: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/plugin-claude/contract")
        .join(format!("{name}.json"));
    let text = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("TERMINESCU_BLESS").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        kept, text,
        "{name}.json differs from what the plugin answers; run with TERMINESCU_BLESS=1"
    );
}

fn hook(plugin: &ClaudePlugin, cx: &Cx, event: Value) {
    plugin.on_event(
        &CoreEvent::Hook {
            pane: "p1".into(),
            event,
        },
        cx,
    );
}

#[test]
fn settings_answers() {
    let base = folder("settings");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane(SHOWN_ROOT);
    let view = plugin.call("settings", json!({}), &cx).unwrap();
    let mut ranges = Map::new();
    let mut errors = Map::new();
    for (key, (lo, hi)) in settings::RANGES {
        ranges.insert((*key).into(), json!([lo, hi]));
        let e = plugin
            .call("setSetting", json!({ "key": key, "value": hi + 1 }), &cx)
            .unwrap_err();
        errors.insert((*key).into(), json!(e.to_string()));
    }
    let (lo, hi) = settings::WINDOW_RANGE;
    ranges.insert(settings::WINDOWS.into(), json!([lo, hi]));
    let e = plugin
        .call(
            "setSetting",
            json!({ "key": settings::WINDOWS, "value": { "claude-opus-5": hi + 1 } }),
            &cx,
        )
        .unwrap_err();
    errors.insert(settings::WINDOWS.into(), json!(e.to_string()));
    let e = plugin
        .call("setSetting", json!({ "key": "color", "value": 1 }), &cx)
        .unwrap_err();
    errors.insert("unknown".into(), json!(e.to_string()));
    check(
        "settings",
        &json!({ "view": view, "ranges": ranges, "errors": errors }),
    );
}

#[test]
fn todos_answers() {
    let base = folder("todos");
    let plugin = ClaudePlugin::new(PathBuf::from("/opt/terminescu"), &base.join("support"));
    let cx = Cx::one_pane(SHOWN_ROOT);
    let none = plugin.call("todos", json!({ "pane": "p1" }), &cx).unwrap();
    let transcript = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks/transcript-insights.jsonl");
    hook(
        &plugin,
        &cx,
        json!({ "hook_event_name": "UserPromptSubmit", "session_id": "sess-1",
                "transcript_path": transcript }),
    );
    let some = plugin.call("todos", json!({ "pane": "p1" }), &cx).unwrap();
    check("todos", &json!({ "none": none, "some": some }));
}

#[test]
fn changes_and_base_text_answers() {
    let base = folder("changes");
    let repo = base.join("shop-api");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("src/retry.ts"), "const tries = 3;\n").unwrap();
    std::fs::write(repo.join("src/old.ts"), "gone\n").unwrap();
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
        assert!(start.elapsed() < Duration::from_secs(10), "no snapshot");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::write(repo.join("src/retry.ts"), "const tries = 5;\nexport {};\n").unwrap();
    std::fs::write(repo.join("src/backoff.ts"), "export const wait = 100;\n").unwrap();
    std::fs::remove_file(repo.join("src/old.ts")).unwrap();
    let changes = plugin
        .call("changes", json!({ "pane": "p1" }), &cx)
        .unwrap();
    let path = |name: &str| repo.join(name).display().to_string();
    let base_of = |name: &str| {
        plugin
            .call("baseText", json!({ "pane": "p1", "path": path(name) }), &cx)
            .unwrap()
    };
    let texts = json!({
        "changed": base_of("src/retry.ts"),
        "created": base_of("src/backoff.ts"),
    });
    let shown = serde_json::to_string(&json!({ "changes": changes, "baseText": texts }))
        .unwrap()
        .replace(&repo.display().to_string(), SHOWN_ROOT);
    check("changes", &serde_json::from_str(&shown).unwrap());
}
