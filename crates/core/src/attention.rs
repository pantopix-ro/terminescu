//! Attention signals (D15, workbench-ui "Attention signals"): which panes wait for the person
//! and since when. Pure: the app feeds agent updates and shows the count, the dock badge and
//! notifications, and `⌘⇧A` jumps to the oldest wait.

use crate::agent::{AgentState, AgentUpdate};
use crate::db::Db;
use crate::state::{Id, Workspace};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SETTINGS_KEY: &str = "attention.settings";

/// Settings › Notifications: the notification is on and the chime off until the person says
/// otherwise. A command that runs `command_secs` or longer posts one when it ends (task 8.40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub notify: bool,
    pub chime: bool,
    pub command_notify: bool,
    pub command_secs: u32,
}

/// The shortest and longest run that may count as long, in seconds.
pub const COMMAND_SECS: std::ops::RangeInclusive<u32> = 5..=3600;

impl Default for Settings {
    fn default() -> Self {
        Self {
            notify: true,
            chime: false,
            command_notify: true,
            command_secs: 30,
        }
    }
}

/// How long a command ran, for its notification: `45s`, `4m 12s`, `1h 3m`.
pub fn run_time(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {}s", s / 60, s % 60),
        _ => format!("{}h {}m", s / 3600, s % 3600 / 60),
    }
}

/// A command as its notification names it: the program and a plain subcommand, as `cargo
/// build` or `npm run`. Leading `NAME=value` assignments, flags and arguments stay out, since
/// they may hold a secret and Notification Center keeps what it shows.
pub fn command_label(command: &str) -> Option<String> {
    let mut words = command
        .split_whitespace()
        .skip_while(|w| w.split_once('=').is_some_and(|(k, _)| is_name(k)));
    let program = words.next()?;
    let program = program.rsplit('/').next().filter(|p| is_word(p))?;
    Some(match words.next().filter(|w| is_subcommand(w)) {
        Some(sub) => format!("{program} {sub}"),
        None => program.to_owned(),
    })
}

/// A subcommand as `build`, `run` or `compose`: lowercase letters, `-` and `:`. A token or an
/// id passed as the first argument has digits or capitals and stays out.
fn is_subcommand(s: &str) -> bool {
    (1..=20).contains(&s.len())
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || "-:".contains(c))
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A short plain word: letters first, then letters, digits, `-`, `_`, `.` or `:`.
fn is_word(s: &str) -> bool {
    s.len() <= 32
        && s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
}

pub fn settings(db: &Db) -> crate::db::Result<Settings> {
    Ok(crate::settings::read(db, SETTINGS_KEY)?.unwrap_or_default())
}

pub fn set_settings(db: &Db, mut s: Settings) -> crate::db::Result<()> {
    s.command_secs = s
        .command_secs
        .clamp(*COMMAND_SECS.start(), *COMMAND_SECS.end());
    crate::settings::write(db, SETTINGS_KEY, &s)
}

#[derive(Debug, Default)]
pub struct Attention {
    /// Waiting pane id to the time it started waiting, in ms.
    waiting: BTreeMap<Id, i64>,
}

/// Where a pane sits, with the names a notification shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub window: Id,
    pub side_tab: Id,
    pub tab: Id,
    pub side_name: String,
    pub tab_name: String,
}

impl Attention {
    /// Applies one agent update. True when the pane starts waiting now, which is when a
    /// notification fires; a pane that keeps waiting keeps its first time.
    pub fn update(&mut self, u: &AgentUpdate, now_ms: i64) -> bool {
        let waiting = u
            .agent
            .as_ref()
            .is_some_and(|a| a.state == AgentState::Waiting);
        if !waiting {
            self.waiting.remove(&u.pane);
            return false;
        }
        if self.waiting.contains_key(&u.pane) {
            return false;
        }
        self.waiting.insert(u.pane.clone(), now_ms);
        true
    }

    pub fn count(&self) -> usize {
        self.waiting.len()
    }

    /// The pane that has waited longest; ties go to the lower id, so the answer is stable.
    pub fn oldest(&self) -> Option<&Id> {
        self.waiting
            .iter()
            .min_by_key(|(id, at)| (**at, (*id).clone()))
            .map(|(id, _)| id)
    }

    /// Forgets panes that no longer exist. True when the count changed.
    pub fn retain(&mut self, ws: &Workspace) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|pane, _| place_of(ws, pane).is_some());
        before != self.waiting.len()
    }
}

/// The window, side tab and tab holding a pane, with their shown names.
pub fn place_of(ws: &Workspace, pane: &str) -> Option<Place> {
    for w in &ws.windows {
        for s in &w.side_tabs {
            for t in &s.tabs {
                if t.layout.has_pane(pane) {
                    let tab_name = t.title.clone().unwrap_or_else(|| "shell".into());
                    let side_name = s
                        .title
                        .clone()
                        .or_else(|| s.tabs.first().and_then(|t| t.title.clone()))
                        .unwrap_or_else(|| "shell".into());
                    return Some(Place {
                        window: w.id.clone(),
                        side_tab: s.id.clone(),
                        tab: t.id.clone(),
                        side_name,
                        tab_name,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentView;

    fn u(pane: &str, state: Option<AgentState>) -> AgentUpdate {
        AgentUpdate {
            pane: pane.into(),
            agent: state.map(|state| AgentView {
                session: "sess-0b5c".into(),
                state,
                last_message: None,
            }),
        }
    }

    #[test]
    fn counts_waits_and_finds_the_oldest() {
        let mut a = Attention::default();
        assert!(a.update(&u("p2", Some(AgentState::Waiting)), 10));
        assert!(a.update(&u("p1", Some(AgentState::Waiting)), 20));
        assert!(
            !a.update(&u("p2", Some(AgentState::Waiting)), 30),
            "still waiting is not new"
        );
        assert_eq!((a.count(), a.oldest().map(String::as_str)), (2, Some("p2")));
        assert!(!a.update(&u("p2", Some(AgentState::Running)), 40));
        assert_eq!((a.count(), a.oldest().map(String::as_str)), (1, Some("p1")));
        a.update(&u("p1", None), 50);
        assert_eq!((a.count(), a.oldest()), (0, None));
    }

    #[test]
    fn settings_default_to_notify_without_chime_and_persist() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(
            settings(&db).unwrap(),
            Settings {
                notify: true,
                chime: false,
                command_notify: true,
                command_secs: 30,
            }
        );
        let s = Settings {
            notify: false,
            chime: true,
            command_notify: false,
            command_secs: 120,
        };
        set_settings(&db, s).unwrap();
        assert_eq!(settings(&db).unwrap(), s);
        // A threshold out of range is brought into it; an old value without the fields reads.
        set_settings(
            &db,
            Settings {
                command_secs: 1,
                ..s
            },
        )
        .unwrap();
        assert_eq!(settings(&db).unwrap().command_secs, 5);
        let old: Settings = serde_json::from_str(r#"{"notify":true,"chime":true}"#).unwrap();
        assert_eq!((old.command_notify, old.command_secs), (true, 30));
    }

    #[test]
    fn run_times_read_as_seconds_minutes_and_hours() {
        assert_eq!(run_time(45_400), "45s");
        assert_eq!(run_time(252_000), "4m 12s");
        assert_eq!(run_time(3_780_000), "1h 3m");
    }

    #[test]
    fn a_notification_names_the_program_not_its_arguments() {
        assert_eq!(
            command_label("cargo build --release").as_deref(),
            Some("cargo build")
        );
        assert_eq!(command_label("npm run dev").as_deref(), Some("npm run"));
        assert_eq!(
            command_label("API_TOKEN=abc123 ./deploy.sh prod").as_deref(),
            Some("deploy.sh prod")
        );
        assert_eq!(
            command_label("curl -H 'Authorization: Bearer x' https://example.com").as_deref(),
            Some("curl")
        );
        assert_eq!(command_label("make").as_deref(), Some("make"));
        // A bare token as the first argument is not a subcommand.
        assert_eq!(command_label("echo tok_AbC123xyz").as_deref(), Some("echo"));
        assert_eq!(
            command_label("vault login hvs.CAESxyz").as_deref(),
            Some("vault login")
        );
        assert_eq!(command_label("open s3cr3t").as_deref(), Some("open"));
        assert_eq!(command_label("  ").as_deref(), None);
        assert_eq!(command_label("'quoted thing'").as_deref(), None);
    }
}
