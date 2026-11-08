//! What a tab's subtitle shows for each terminal pane (task 8.30): the pane's folder from home,
//! the program it runs and, while Claude runs there, Claude's title. Only names leave here; a
//! command line's arguments can hold a token, so they never reach the UI.

use serde::Serialize;

/// The default title Claude Code shows before a session has one.
const CLAUDE_DEFAULT: &str = "Claude Code";

/// Longest program name kept, in characters.
const MAX_PROGRAM: usize = 40;

/// Words that run another program: the program is the next word that is not a flag.
const WRAPPERS: &[&str] = &[
    "sudo",
    "env",
    "exec",
    "time",
    "nohup",
    "command",
    "builtin",
    "nice",
    "caffeinate",
    "noglob",
];

/// One terminal pane's live label parts. Never saved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneLabel {
    /// The cwd, with the home folder written `~`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The running command's program name, without its arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// Claude's title while Claude runs in the pane, without its spinner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The pty's foreground process, as Terminal.app names a tab: `zsh` at the prompt
    /// (task 8.62). A tab is named after it while no command of its own runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

/// A process name fit for a tab: no control characters, at most [`MAX_PROGRAM`] characters,
/// none for a blank one.
pub fn process_name(raw: &str) -> Option<String> {
    let name: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_PROGRAM)
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// `cwd` with `home` written `~`; other paths stay whole.
pub fn home_path(cwd: &str, home: Option<&str>) -> String {
    let Some(home) = home
        .map(|h| h.trim_end_matches('/'))
        .filter(|h| !h.is_empty())
    else {
        return cwd.to_string();
    };
    if cwd == home {
        return "~".into();
    }
    match cwd.strip_prefix(home) {
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => cwd.to_string(),
    }
}

/// The program a command line runs: the first word that is no `NAME=value`, past wrappers
/// such as `sudo` and their flags, without its folder. None for an empty line.
pub fn program_of(command: &str) -> Option<String> {
    let mut wrapped = false;
    for word in command.split_whitespace() {
        if word.starts_with('-') && wrapped {
            continue;
        }
        if is_assignment(word) {
            continue;
        }
        let name = word.rsplit('/').next().unwrap_or(word);
        if WRAPPERS.contains(&name) {
            wrapped = true;
            continue;
        }
        let name: String = name.chars().take(MAX_PROGRAM).collect();
        return (!name.is_empty()).then_some(name);
    }
    None
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A pane's label from what the core knows of it. `claude` says a Claude session runs there.
pub fn label(
    cwd: Option<&str>,
    running: bool,
    command: Option<&str>,
    title: Option<&str>,
    claude: bool,
    home: Option<&str>,
) -> PaneLabel {
    PaneLabel {
        path: cwd.map(|c| home_path(c, home)),
        program: running.then(|| command.and_then(program_of)).flatten(),
        title: claude
            .then_some(title)
            .flatten()
            .filter(|t| *t != CLAUDE_DEFAULT)
            .map(str::to_string),
        process: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_under_home_starts_with_a_tilde() {
        let home = Some("/Users/owner");
        assert_eq!(home_path("/Users/owner/work/api", home), "~/work/api");
        assert_eq!(home_path("/Users/owner", home), "~");
        assert_eq!(home_path("/Users/owner2/x", home), "/Users/owner2/x");
        assert_eq!(home_path("/tmp", home), "/tmp");
        assert_eq!(home_path("/tmp", None), "/tmp");
    }

    #[test]
    fn a_program_never_carries_its_arguments() {
        assert_eq!(program_of("npm run dev").as_deref(), Some("npm"));
        assert_eq!(
            program_of("TOKEN=tok-example curl -H x").as_deref(),
            Some("curl")
        );
        assert_eq!(
            program_of("sudo -E /usr/bin/vim /etc/hosts").as_deref(),
            Some("vim")
        );
        assert_eq!(
            program_of("env A=1 B=2 node app.js").as_deref(),
            Some("node")
        );
        assert_eq!(program_of("time cargo test").as_deref(), Some("cargo"));
        assert_eq!(program_of("   ").as_deref(), None);
        assert_eq!(program_of("A=1").as_deref(), None);
    }

    #[test]
    fn a_process_name_drops_control_characters_and_stays_short() {
        assert_eq!(process_name("zsh").as_deref(), Some("zsh"));
        assert_eq!(process_name("np\u{7}m").as_deref(), Some("npm"));
        assert_eq!(process_name(" \n").as_deref(), None);
        assert_eq!(
            process_name(&"x".repeat(99)).map(|n| n.len()),
            Some(MAX_PROGRAM)
        );
    }

    #[test]
    fn claude_title_shows_only_while_claude_runs_and_not_the_default() {
        let home = Some("/Users/owner");
        let l = label(
            Some("/Users/owner/api"),
            true,
            Some("claude --resume x"),
            Some("Fix the export"),
            true,
            home,
        );
        assert_eq!(l.path.as_deref(), Some("~/api"));
        assert_eq!(l.program.as_deref(), Some("claude"));
        assert_eq!(l.title.as_deref(), Some("Fix the export"));
        assert_eq!(
            label(None, true, None, Some("Claude Code"), true, home).title,
            None
        );
        assert_eq!(
            label(None, true, Some("vim"), Some("vim"), false, home).title,
            None
        );
        assert_eq!(
            label(None, false, Some("npm test"), None, false, home).program,
            None
        );
    }
}
