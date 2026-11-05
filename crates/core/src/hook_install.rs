//! Registers `terminescu hook` in Claude Code's user settings (task 4.1, D34, D36). The edit is
//! idempotent: every earlier Terminescu entry goes, one fresh entry per event comes in, and every
//! other hook and setting stays as it was, key order included.

use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The events D34 listens to. `PreToolUse` stays out: it fires before the prompt and tells
/// nothing the other events don't.
pub const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// Seconds Claude Code waits for the hook; it returns in under 100 ms (D34).
pub const TIMEOUT_SECS: u64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0} is not valid JSON, so Terminescu leaves it alone: {1}")]
    NotJson(PathBuf, serde_json::Error),
    #[error("{0} holds {1} where an object belongs, so Terminescu leaves it alone")]
    Shape(PathBuf, &'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `settings.json` in `CLAUDE_CONFIG_DIR` when set, else `~/.claude/settings.json`.
pub fn user_settings() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("settings.json");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude/settings.json")
}

/// The hook command: the CLI path in single quotes, since it holds a space (D36) and the shell
/// must not expand anything in it.
pub fn command_for(cli: &Path) -> String {
    format!(
        "'{}' hook",
        cli.display().to_string().replace('\'', r"'\''")
    )
}

/// A command that runs some `terminescu` binary's `hook`, wherever it was installed.
fn is_ours(command: &str) -> bool {
    let Some(program) = command.trim().strip_suffix(" hook") else {
        return false;
    };
    let program = program.trim().trim_matches(|c| c == '"' || c == '\'');
    Path::new(program)
        .file_name()
        .is_some_and(|n| n == "terminescu")
}

/// Rewrites `settings` in place; returns true when anything changed.
pub fn apply(settings: &mut Map<String, Value>, command: &str) -> Result<bool, &'static str> {
    let before = Value::Object(settings.clone());
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(hooks) = hooks else {
        return Err("a non-object \"hooks\"");
    };
    // Drop every Terminescu hook, then the groups and events that end up empty because of it.
    for (_, groups) in hooks.iter_mut() {
        let Value::Array(groups) = groups else {
            continue;
        };
        groups.retain_mut(|group| {
            let Some(Value::Array(list)) = group.get_mut("hooks") else {
                return true;
            };
            let had = list.len();
            list.retain(|h| {
                !h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(is_ours)
            });
            had == list.len() || !list.is_empty()
        });
    }
    hooks.retain(|_, groups| !matches!(groups, Value::Array(g) if g.is_empty()));
    for event in EVENTS {
        let groups = hooks
            .entry(event.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Value::Array(groups) = groups else {
            return Err("a non-array hook event");
        };
        groups.push(json!({
            "hooks": [{ "type": "command", "command": command, "timeout": TIMEOUT_SECS }]
        }));
    }
    Ok(!same_order(&before, settings))
}

/// Equal values in the same key order: what a rewrite of the file would show.
fn same_order(before: &Value, after: &Map<String, Value>) -> bool {
    serde_json::to_string(before).ok() == serde_json::to_string(after).ok()
}

/// Installs the hook into the settings file at `path`, creating it when missing. The write goes
/// through a temporary file and a rename, so Claude never reads half a file.
pub fn install(path: &Path, cli: &Path) -> Result<bool, InstallError> {
    // A settings file linked from a dotfiles folder: write the real file, keep the link.
    let real = std::fs::canonicalize(path).ok();
    let path = real.as_deref().unwrap_or(path);
    let mut settings = match std::fs::read(path) {
        Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Map::new(),
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(InstallError::Shape(path.into(), "a non-object top level")),
            Err(e) => return Err(InstallError::NotJson(path.into(), e)),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(e.into()),
    };
    let changed = apply(&mut settings, &command_for(cli))
        .map_err(|what| InstallError::Shape(path.into(), what))?;
    if !changed {
        return Ok(false);
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut text = serde_json::to_string_pretty(&settings).expect("a JSON map serializes");
    text.push('\n');
    let tmp = dir.join(format!(".settings.json.terminescu-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    if let Ok(meta) = std::fs::metadata(path) {
        std::fs::set_permissions(&tmp, meta.permissions())?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLI: &str = "/Users/owner/Library/Application Support/Terminescu/bin/terminescu";

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("terminescu-hooks-{}", crate::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    fn count_ours(v: &Value, event: &str) -> usize {
        v["hooks"][event]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .filter(|h| is_ours(h["command"].as_str().unwrap()))
            .count()
    }

    #[test]
    fn running_twice_leaves_one_entry_per_event_and_other_hooks_unchanged() {
        let path = temp();
        let theirs = json!({
            "model": "opus",
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }],
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "audit.sh" }] }]
            },
            "permissions": { "allow": ["Bash(ls:*)"] }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&theirs).unwrap()).unwrap();
        assert!(install(&path, Path::new(CLI)).unwrap());
        let once = std::fs::read_to_string(&path).unwrap();
        assert!(
            !install(&path, Path::new(CLI)).unwrap(),
            "the second run changes nothing"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);

        let v: Value = serde_json::from_str(&once).unwrap();
        for event in EVENTS {
            assert_eq!(count_ours(&v, event), 1, "{event}");
        }
        assert_eq!(v["hooks"]["Stop"][0], theirs["hooks"]["Stop"][0]);
        assert_eq!(v["hooks"]["PreToolUse"], theirs["hooks"]["PreToolUse"]);
        assert_eq!(v["permissions"], theirs["permissions"]);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "permissions"], "key order stays");
        let ours = &v["hooks"]["SessionStart"][0]["hooks"][0];
        assert_eq!(ours["command"], format!("'{CLI}' hook"));
        assert_eq!(ours["timeout"], 5);
    }

    #[test]
    fn a_moved_cli_replaces_the_old_entry() {
        let path = temp();
        install(&path, Path::new("/old/place/terminescu")).unwrap();
        install(&path, Path::new(CLI)).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(count_ours(&v, "Stop"), 1);
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("/old/place")
        );
    }

    #[test]
    fn a_missing_file_is_created_and_a_broken_one_left_alone() {
        let path = temp();
        assert!(install(&path, Path::new(CLI)).unwrap());
        assert!(path.exists());
        std::fs::write(&path, "{ not json").unwrap();
        assert!(matches!(
            install(&path, Path::new(CLI)),
            Err(InstallError::NotJson(..))
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn a_linked_settings_file_keeps_its_link() {
        let path = temp();
        let real = path.with_file_name("dotfiles-settings.json");
        std::fs::write(&real, "{}").unwrap();
        std::os::unix::fs::symlink(&real, &path).unwrap();
        assert!(install(&path, Path::new(CLI)).unwrap());
        assert!(std::fs::symlink_metadata(&path).unwrap().is_symlink());
        assert!(std::fs::read_to_string(&real).unwrap().contains("hook"));
    }

    #[test]
    fn only_terminescu_hook_commands_count_as_ours() {
        assert!(is_ours(&command_for(Path::new(CLI))));
        assert!(is_ours("/usr/local/bin/terminescu hook"));
        assert!(!is_ours("/usr/local/bin/terminescu status done"));
        assert!(!is_ours("my-terminescu hook"));
        assert!(!is_ours("say hook"));
    }
}
