//! Statuses (D15, tasks 5.1 and 5.3). Each tab and side tab shows one status. A manual status
//! beats a dynamic one (set by a rule or by `terminescu status`), which beats the auto status
//! that hooks and the shell integration produce. The amber needs-you marker shows over all of
//! them. Pure: the runtime feeds facts and events in and sends the result to the UI.

use crate::state::{Id, Workspace};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use terminescu_plugin_api::AgentState;
use terminescu_protocol::ShellState;

/// Accent hues a status may take. Amber stays reserved for "needs you" (D16).
pub const ACCENTS: &[&str] = &["blue", "cyan", "green", "magenta", "red", "fg"];
/// Background tints a status may take.
pub const TINTS: &[&str] = &["blue", "cyan", "green", "magenta", "red"];
/// Longest label, in characters.
pub const MAX_LABEL: usize = 24;

/// The built-in statuses, least urgent first, so `max` picks the most urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Auto {
    /// A shell at the prompt with no command run yet, or Claude before its first prompt.
    Idle,
    /// The last command exited non-zero.
    Failed,
    /// Claude finished its turn, or the last command exited 0.
    Done,
    /// A command runs, or Claude works.
    Running,
    /// Claude needs the person.
    Waiting,
}

/// A status's face: label, accent and background.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    pub label: String,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatusError {
    #[error("the label is empty")]
    EmptyLabel,
    #[error("the label is longer than {MAX_LABEL} characters")]
    LongLabel,
    #[error("`{0}` is not an accent; pick one of blue, cyan, green, magenta, red, fg")]
    BadAccent(String),
    #[error("`{0}` is not a background; pick one of blue, cyan, green, magenta, red")]
    BadBackground(String),
}

impl Look {
    pub fn check(&self) -> Result<(), StatusError> {
        let label = self.label.trim();
        if label.is_empty() {
            return Err(StatusError::EmptyLabel);
        }
        if label.chars().count() > MAX_LABEL {
            return Err(StatusError::LongLabel);
        }
        if let Some(a) = &self.accent
            && !ACCENTS.contains(&a.as_str())
        {
            return Err(StatusError::BadAccent(a.clone()));
        }
        if let Some(b) = &self.background
            && !TINTS.contains(&b.as_str())
        {
            return Err(StatusError::BadBackground(b.clone()));
        }
        Ok(())
    }
}

/// A status set by hand on a tab or side tab. `status_id` names the saved status it came
/// from, so editing or deleting that status reaches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_id: Option<Id>,
    #[serde(flatten)]
    pub look: Look,
}

/// What a rule watches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RuleKind {
    /// A command matching `pattern` (`*` and `?` wildcards) exits non-zero; holds until that
    /// command runs again.
    ExitCode { pattern: String },
    /// Every Claude session in the pane sat Done or Idle this long; holds until activity.
    ClaudeIdle { minutes: u32 },
    /// A plugin fact such as `git.conflict` holds on the pane, its tab or its row; holds while
    /// it does (add-plugin-architecture D4).
    Fact { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: Id,
    pub status_id: Id,
    pub look: Look,
    #[serde(flatten)]
    pub kind: RuleKind,
    pub enabled: bool,
}

impl Rule {
    fn matches(&self, command: Option<&str>) -> bool {
        match (&self.kind, command) {
            (RuleKind::ExitCode { pattern }, Some(c)) => glob(pattern.trim(), c.trim()),
            _ => false,
        }
    }
}

/// `*` matches any run of characters, `?` one character; the whole text must match.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Rule(Id),
    Cli,
}

/// When a dynamic status ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Never,
    /// At this Unix ms (`--for`).
    At(i64),
    /// When the pane's running command ends (`--until-exit`).
    CommandEnd,
    /// When the rule's command starts again.
    NextRun,
    /// When the pane's Claude session works again.
    Activity,
    /// When the rule's plugin fact stops holding.
    Fact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dynamic {
    pub look: Look,
    pub origin: Origin,
    pub until: Until,
    /// Bigger is newer: the newest dynamic status of a tab shows.
    pub order: u64,
}

/// What a tab or side tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Manual,
    /// A launcher side tab's `n/m up` (D24).
    Launcher,
    Rule,
    Cli,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    /// What the hooks and the shell say, shown when nothing overrides it.
    pub auto: Auto,
    /// A Claude session inside waits: the amber marker shows over any status.
    pub waiting: bool,
    pub source: Source,
    /// The label and colors, absent when `source` is `auto` (the UI names built-ins).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<Look>,
}

/// Manual, then dynamic, then auto (D15).
pub fn resolve(
    manual: Option<&Look>,
    dynamic: Option<&Dynamic>,
    auto: Auto,
    waiting: bool,
) -> Shown {
    let (source, look) = match (manual, dynamic) {
        (Some(m), _) => (Source::Manual, Some(m.clone())),
        (None, Some(d)) => (
            match d.origin {
                Origin::Rule(_) => Source::Rule,
                Origin::Cli => Source::Cli,
            },
            Some(d.look.clone()),
        ),
        (None, None) => (Source::Auto, None),
    };
    Shown {
        auto,
        waiting,
        source,
        look,
    }
}

/// A pane's auto status. A Claude session in the pane decides it; otherwise the shell does.
pub fn pane_auto(agent: Option<AgentState>, shell: Option<&ShellState>) -> Auto {
    if let Some(a) = agent {
        return match a {
            AgentState::Waiting => Auto::Waiting,
            AgentState::Running => Auto::Running,
            AgentState::Done => Auto::Done,
            AgentState::Idle => Auto::Idle,
        };
    }
    match shell {
        Some(s) if s.running => Auto::Running,
        Some(ShellState {
            last_exit: Some(0), ..
        }) => Auto::Done,
        Some(ShellState {
            last_exit: Some(_), ..
        }) => Auto::Failed,
        _ => Auto::Idle,
    }
}

/// The dynamic statuses per pane and the rules that set them.
#[derive(Debug, Default)]
pub struct Engine {
    rules: Vec<Rule>,
    dynamic: HashMap<Id, Vec<Dynamic>>,
    order: u64,
}

impl Engine {
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn dynamic(&self, pane: &str) -> &[Dynamic] {
        self.dynamic.get(pane).map(Vec::as_slice).unwrap_or(&[])
    }

    fn put(&mut self, pane: &str, look: Look, origin: Origin, until: Until) {
        self.order += 1;
        let list = self.dynamic.entry(pane.into()).or_default();
        list.retain(|d| d.origin != origin);
        list.push(Dynamic {
            look,
            origin,
            until,
            order: self.order,
        });
    }

    fn drop_where(&mut self, pane: &str, gone: impl Fn(&Dynamic) -> bool) -> bool {
        let Some(list) = self.dynamic.get_mut(pane) else {
            return false;
        };
        let before = list.len();
        list.retain(|d| !gone(d));
        let changed = list.len() != before;
        if list.is_empty() {
            self.dynamic.remove(pane);
        }
        changed
    }

    /// New rules from Settings. A status a deleted or disabled rule set goes; the others take
    /// the rule's current look.
    pub fn set_rules(&mut self, rules: Vec<Rule>) {
        self.rules = rules;
        let rules = &self.rules;
        for list in self.dynamic.values_mut() {
            list.retain_mut(|d| match &d.origin {
                Origin::Cli => true,
                Origin::Rule(id) => match rules.iter().find(|r| &r.id == id && r.enabled) {
                    Some(r) => {
                        d.look = r.look.clone();
                        true
                    }
                    None => false,
                },
            });
        }
        self.dynamic.retain(|_, l| !l.is_empty());
    }

    /// `terminescu status` from a pane: sets its status, or clears it with `None`.
    pub fn cli(&mut self, pane: &str, look: Option<Look>, until: Until) {
        match look {
            Some(look) => self.put(pane, look, Origin::Cli, until),
            None => {
                self.drop_where(pane, |d| d.origin == Origin::Cli);
            }
        }
    }

    /// A pane's shell state moved from `prev` to `next`. Returns whether a status changed.
    pub fn shell(&mut self, pane: &str, prev: Option<&ShellState>, next: &ShellState) -> bool {
        let empty = ShellState::default();
        let prev = prev.unwrap_or(&empty);
        // The run counters see every run; a daemon without them leaves the running flag.
        let counted = next.started + next.ended + prev.started + prev.ended > 0;
        let started = if counted {
            next.started > prev.started
        } else {
            !prev.running && next.running
        };
        let ended = if counted {
            next.ended > prev.ended
        } else {
            prev.running && !next.running && next.last_exit.is_some()
        };
        let command = next.command.as_deref();
        let mut changed = false;
        if started {
            let rerun: Vec<Id> = self
                .rules
                .iter()
                .filter(|r| r.matches(command))
                .map(|r| r.id.clone())
                .collect();
            changed |= self.drop_where(pane, |d| {
                d.until == Until::NextRun
                    && matches!(&d.origin, Origin::Rule(id) if rerun.contains(id))
            });
        }
        if ended {
            changed |= self.drop_where(pane, |d| d.until == Until::CommandEnd);
            if next.last_exit.is_some_and(|c| c != 0) {
                let hits: Vec<Rule> = self
                    .rules
                    .iter()
                    .filter(|r| r.enabled && r.matches(command))
                    .cloned()
                    .collect();
                for r in hits {
                    self.put(pane, r.look, Origin::Rule(r.id), Until::NextRun);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Time passes: `--for` statuses expire and Claude idle rules fire or clear. `idle_since`
    /// holds, per pane with a Claude session, since when it sat Done or Idle (None while it
    /// works or waits). Returns whether a status changed.
    pub fn tick(&mut self, now: i64, idle_since: &HashMap<Id, Option<i64>>) -> bool {
        let mut changed = false;
        let panes: Vec<Id> = self.dynamic.keys().cloned().collect();
        for pane in &panes {
            changed |= self.drop_where(pane, |d| matches!(d.until, Until::At(t) if t <= now));
        }
        let idle_rules: Vec<(Rule, u32)> = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| match r.kind {
                RuleKind::ClaudeIdle { minutes } => Some((r.clone(), minutes)),
                _ => None,
            })
            .collect();
        for (pane, since) in idle_since {
            match since {
                None => changed |= self.drop_where(pane, |d| d.until == Until::Activity),
                Some(t) => {
                    for (r, minutes) in &idle_rules {
                        let due = now - t >= i64::from(*minutes) * 60_000;
                        let has = self
                            .dynamic(pane)
                            .iter()
                            .any(|d| d.origin == Origin::Rule(r.id.clone()));
                        if due && !has {
                            self.put(
                                pane,
                                r.look.clone(),
                                Origin::Rule(r.id.clone()),
                                Until::Activity,
                            );
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
    }

    /// Fact rules against the facts that hold now, by pane. Returns whether a status changed.
    pub fn facts(&mut self, holding: &HashMap<Id, HashSet<String>>) -> bool {
        let rules: Vec<(Rule, String)> = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| match &r.kind {
                RuleKind::Fact { name } => Some((r.clone(), name.clone())),
                _ => None,
            })
            .collect();
        let mut changed = false;
        let panes: HashSet<Id> = holding.keys().chain(self.dynamic.keys()).cloned().collect();
        for pane in &panes {
            let held = holding.get(pane);
            for (r, name) in &rules {
                let origin = Origin::Rule(r.id.clone());
                let has = self.dynamic(pane).iter().any(|d| d.origin == origin);
                let holds = held.is_some_and(|h| h.contains(name));
                if holds && !has {
                    self.put(pane, r.look.clone(), origin, Until::Fact);
                    changed = true;
                } else if !holds && has {
                    changed |= self.drop_where(pane, |d| d.origin == origin);
                }
            }
        }
        changed
    }

    /// Forgets the statuses of panes that no longer exist.
    /// The pane's shell was replaced: statuses tied to its runs go, CLI `--for` ones stay.
    pub fn shell_restarted(&mut self, pane: &str) -> bool {
        self.drop_where(pane, |d| {
            matches!(d.until, Until::CommandEnd | Until::NextRun)
        })
    }

    pub fn retain_panes(&mut self, live: &HashSet<&str>) {
        self.dynamic.retain(|p, _| live.contains(p.as_str()));
    }

    fn newest<'a>(&'a self, panes: impl Iterator<Item = &'a Id>) -> Option<&'a Dynamic> {
        panes
            .flat_map(|p| self.dynamic(p).iter())
            .max_by_key(|d| d.order)
    }
}

/// A launcher side tab's look: how many of its commands run, green when all do, red otherwise.
pub fn launcher_look(up: usize, total: usize) -> Look {
    Look {
        label: format!("{up}/{total} up"),
        accent: Some(if up == total { "green" } else { "red" }.into()),
        background: None,
    }
}

/// Gives each launcher side tab its `n/m up` status, under a manual one (D24). `launchers`
/// maps a side tab to the tab of each launcher command, `None` when that tab is gone; a
/// command is up while its tab's auto status is Running.
pub fn apply_launchers(map: &mut BTreeMap<Id, Shown>, launchers: &HashMap<Id, Vec<Option<Id>>>) {
    for (side, tabs) in launchers {
        if tabs.is_empty() || map.get(side).is_none_or(|s| s.source == Source::Manual) {
            continue;
        }
        let up = tabs
            .iter()
            .flatten()
            .filter(|t| map.get(*t).is_some_and(|s| s.auto == Auto::Running))
            .count();
        if let Some(shown) = map.get_mut(side) {
            shown.source = Source::Launcher;
            shown.look = Some(launcher_look(up, tabs.len()));
        }
    }
}

/// What the runtime knows about one terminal pane.
#[derive(Debug, Clone, Default)]
pub struct PaneFacts {
    pub agent: Option<AgentState>,
    pub shell: Option<ShellState>,
}

/// Every tab's and side tab's status, by id. A side tab shows its own manual status, else the
/// newest dynamic status of its panes, else its most urgent tab's auto status.
pub fn compute(
    ws: &Workspace,
    facts: &HashMap<Id, PaneFacts>,
    engine: &Engine,
) -> BTreeMap<Id, Shown> {
    let mut out = BTreeMap::new();
    for side in ws.windows.iter().flat_map(|w| w.side_tabs.iter()) {
        let mut side_auto = Auto::Idle;
        let mut side_waiting = false;
        let mut side_panes: Vec<&Id> = Vec::new();
        for tab in &side.tabs {
            let panes = tab.layout.terminals();
            let pane_facts = panes.iter().filter_map(|p| facts.get(*p));
            let auto = pane_facts
                .clone()
                .map(|f| pane_auto(f.agent, f.shell.as_ref()))
                .max()
                .unwrap_or(Auto::Idle);
            let waiting = pane_facts
                .clone()
                .any(|f| f.agent == Some(AgentState::Waiting));
            let dynamic = engine.newest(panes.iter().copied());
            let manual = tab.status.as_ref().map(|m| &m.look);
            out.insert(tab.id.clone(), resolve(manual, dynamic, auto, waiting));
            side_auto = side_auto.max(auto);
            side_waiting |= waiting;
            side_panes.extend(panes);
        }
        let dynamic = engine.newest(side_panes.into_iter());
        let manual = side.status.as_ref().map(|m| &m.look);
        out.insert(
            side.id.clone(),
            resolve(manual, dynamic, side_auto, side_waiting),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_launcher_counts_running_tabs_under_a_manual_status() {
        let shown = |auto, source| Shown {
            auto,
            waiting: false,
            source,
            look: None,
        };
        let mut map = BTreeMap::from([
            ("s1".to_string(), shown(Auto::Running, Source::Auto)),
            ("t1".to_string(), shown(Auto::Running, Source::Auto)),
            ("t2".to_string(), shown(Auto::Failed, Source::Auto)),
            ("s2".to_string(), shown(Auto::Idle, Source::Manual)),
        ]);
        let launchers = HashMap::from([
            (
                "s1".to_string(),
                vec![Some("t1".to_string()), Some("t2".to_string()), None],
            ),
            ("s2".to_string(), vec![Some("t1".to_string())]),
        ]);
        apply_launchers(&mut map, &launchers);
        assert_eq!(map["s1"].source, Source::Launcher);
        assert_eq!(map["s1"].look, Some(launcher_look(1, 3)));
        assert_eq!(launcher_look(1, 3).accent.as_deref(), Some("red"));
        assert_eq!(launcher_look(2, 2).accent.as_deref(), Some("green"));
        assert_eq!(map["s2"].source, Source::Manual, "a manual status stays");
    }

    use super::*;

    #[test]
    fn urgency_runs_waiting_running_done_failed_idle() {
        let mut all = vec![
            Auto::Idle,
            Auto::Done,
            Auto::Waiting,
            Auto::Failed,
            Auto::Running,
        ];
        all.sort_by(|a, b| b.cmp(a));
        assert_eq!(
            all,
            [
                Auto::Waiting,
                Auto::Running,
                Auto::Done,
                Auto::Failed,
                Auto::Idle
            ]
        );
    }

    #[test]
    fn a_pane_follows_its_claude_session_then_its_shell() {
        let ran = |code| ShellState {
            last_exit: Some(code),
            ended: 1,
            started: 1,
            ..Default::default()
        };
        assert_eq!(pane_auto(None, None), Auto::Idle);
        assert_eq!(pane_auto(None, Some(&ran(0))), Auto::Done);
        assert_eq!(pane_auto(None, Some(&ran(1))), Auto::Failed);
        let running = ShellState {
            running: true,
            ..Default::default()
        };
        assert_eq!(pane_auto(None, Some(&running)), Auto::Running);
        assert_eq!(
            pane_auto(Some(AgentState::Waiting), Some(&running)),
            Auto::Waiting
        );
        assert_eq!(
            pane_auto(Some(AgentState::Idle), Some(&running)),
            Auto::Idle
        );
    }

    #[test]
    fn globs_match_the_whole_command() {
        assert!(glob("pnpm test*", "pnpm test"));
        assert!(glob("pnpm test*", "pnpm test --filter api"));
        assert!(!glob("pnpm test*", "pnpm lint && pnpm test"));
        assert!(glob("cargo ?est", "cargo test"));
        assert!(glob("*deploy*", "make deploy-staging"));
        assert!(!glob("make", "make deploy"));
    }

    #[test]
    fn looks_refuse_amber_and_long_labels() {
        let look = |label: &str, accent: Option<&str>| Look {
            label: label.into(),
            accent: accent.map(String::from),
            background: None,
        };
        assert!(look("Review", Some("cyan")).check().is_ok());
        assert_eq!(
            look("Review", Some("amber")).check(),
            Err(StatusError::BadAccent("amber".into()))
        );
        assert_eq!(look("  ", None).check(), Err(StatusError::EmptyLabel));
        assert_eq!(
            look(&"x".repeat(MAX_LABEL + 1), None).check(),
            Err(StatusError::LongLabel)
        );
    }
}
