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
/// otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub notify: bool,
    pub chime: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notify: true,
            chime: false,
        }
    }
}

pub fn settings(db: &Db) -> crate::db::Result<Settings> {
    Ok(crate::settings::read(db, SETTINGS_KEY)?.unwrap_or_default())
}

pub fn set_settings(db: &Db, s: Settings) -> crate::db::Result<()> {
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
                chime: false
            }
        );
        let s = Settings {
            notify: false,
            chime: true,
        };
        set_settings(&db, s).unwrap();
        assert_eq!(settings(&db).unwrap(), s);
    }
}
