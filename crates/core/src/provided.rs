//! What plugins provide to the core (add-plugin-architecture D4): named facts that dynamic
//! status rules test, and read-only prompt variables. Both live in memory and carry the plugin
//! that set them; a fact is named `plugin.name`, so plugins never share one.

use crate::context::{Mode, Variable};
use crate::state::{Id, Workspace};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use terminescu_plugin_api::Target;

/// `plugin.name`: lowercase plugin id, a dot, then letters, digits and `_`.
pub fn is_fact_name(name: &str) -> bool {
    let Some((plugin, rest)) = name.split_once('.') else {
        return false;
    };
    let word = |w: &str, ok: fn(char) -> bool| !w.is_empty() && w.len() <= 64 && w.chars().all(ok);
    word(plugin, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }) && word(rest, |c| c.is_ascii_alphanumeric() || c == '_')
}

/// A fact holds unless it is null, false, zero or an empty string.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[derive(Debug, Default)]
pub struct Provided {
    facts: HashMap<(Target, String), Value>,
    /// By target and name: the plugin and the value.
    variables: HashMap<(Target, String), (String, String)>,
}

impl Provided {
    /// Sets `plugin.name` on a target; `None` clears it. True when anything changed.
    pub fn set_fact(
        &mut self,
        plugin: &str,
        target: &Target,
        name: &str,
        v: Option<Value>,
    ) -> bool {
        let key = (target.clone(), format!("{plugin}.{name}"));
        match v {
            Some(v) => self.facts.insert(key, v.clone()).as_ref() != Some(&v),
            None => self.facts.remove(&key).is_some(),
        }
    }

    pub fn set_variable(&mut self, plugin: &str, target: &Target, name: &str, v: Option<String>) {
        let key = (target.clone(), name.to_string());
        match v {
            Some(v) => {
                self.variables.insert(key, (plugin.into(), v));
            }
            None => {
                self.variables.remove(&key);
            }
        }
    }

    /// Drops what a plugin set, when it stops. True when a fact went.
    pub fn forget_plugin(&mut self, plugin: &str) -> bool {
        let prefix = format!("{plugin}.");
        let before = self.facts.len();
        self.facts.retain(|(_, n), _| !n.starts_with(&prefix));
        self.variables.retain(|_, (p, _)| p != plugin);
        self.facts.len() != before
    }

    /// Drops what was set on targets that are gone. True when a fact went.
    pub fn forget(&mut self, gone: &Target) -> bool {
        let before = self.facts.len();
        self.facts.retain(|(t, _), _| t != gone);
        self.variables.retain(|(t, _), _| t != gone);
        self.facts.len() != before
    }

    /// The facts that hold per terminal pane: its own, its tab's and its row's.
    pub fn holding(&self, ws: &Workspace) -> HashMap<Id, HashSet<String>> {
        let mut out: HashMap<Id, HashSet<String>> = HashMap::new();
        if self.facts.is_empty() {
            return out;
        }
        for side in ws.windows.iter().flat_map(|w| w.side_tabs.iter()) {
            for tab in &side.tabs {
                for pane in tab.layout.terminals() {
                    let on = [
                        Target::Pane(pane.clone()),
                        Target::Tab(tab.id.clone()),
                        Target::SideTab(side.id.clone()),
                    ];
                    let names: HashSet<String> = self
                        .facts
                        .iter()
                        .filter(|((t, _), v)| on.contains(t) && truthy(v))
                        .map(|((_, n), _)| n.clone())
                        .collect();
                    if !names.is_empty() {
                        out.insert(pane.clone(), names);
                    }
                }
            }
        }
        out
    }

    /// The row's provided variables after `saved`, as read-only session variables. A name the
    /// row already has wins; among provided ones the row's own come first, then its tabs' and
    /// panes' in workspace order.
    pub fn with_variables(
        &self,
        ws: &Workspace,
        side_tab: &str,
        saved: Vec<Variable>,
    ) -> Vec<Variable> {
        let Some(side) = ws.side_tab(side_tab) else {
            return saved;
        };
        let mut targets = vec![Target::SideTab(side.id.clone())];
        for tab in &side.tabs {
            targets.push(Target::Tab(tab.id.clone()));
            targets.extend(
                tab.layout
                    .terminals()
                    .into_iter()
                    .map(|p| Target::Pane(p.clone())),
            );
        }
        let mut taken: HashSet<String> = saved.iter().map(|v| v.name.clone()).collect();
        let mut out = saved;
        for t in &targets {
            let mut here: Vec<(&String, &(String, String))> = self
                .variables
                .iter()
                .filter(|((vt, _), _)| vt == t)
                .map(|((_, n), pv)| (n, pv))
                .collect();
            here.sort_by(|a, b| a.0.cmp(b.0));
            for (name, (plugin, value)) in here {
                if !taken.insert(name.clone()) {
                    continue;
                }
                out.push(Variable {
                    id: format!("plugin:{plugin}:{name}"),
                    side_tab_id: side.id.clone(),
                    name: name.clone(),
                    value: Some(value.clone()),
                    mode: Mode::Session,
                    source: Some(format!("plugin:{plugin}")),
                    origin_tab_id: None,
                    expires_at: None,
                });
            }
        }
        out
    }
}

/// True for a variable a plugin provides, which the person cannot edit or remove.
pub fn is_provided(v: &Variable) -> bool {
    v.id.starts_with("plugin:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::db::Db;
    use crate::persist;
    use crate::state::{Command, Core};
    use crate::status::{self, Engine, Look, PaneFacts, Rule, RuleKind, Source};

    fn core() -> Core {
        let mut n = 0;
        let mut c = Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        );
        c.handle(Command::CreateWindow).unwrap();
        let window = c.workspace().windows[0].id.clone();
        c.handle(Command::CreateSideTab {
            window_id: window,
            index: None,
        })
        .unwrap();
        c
    }

    #[test]
    fn fact_names_read_plugin_dot_name() {
        assert!(is_fact_name("git.conflict"));
        assert!(is_fact_name("git.checks_failed"));
        assert!(!is_fact_name("conflict"));
        assert!(!is_fact_name("Git.x"));
        assert!(!is_fact_name("git."));
        assert!(!is_fact_name("git.a b"));
    }

    #[test]
    fn a_plugin_fact_drives_a_dynamic_status() {
        let c = core();
        let ws = c.workspace().clone();
        let side = &ws.windows[0].side_tabs[0];
        let tab = &side.tabs[0];
        let pane = tab.layout.terminals()[0].clone();
        let mut engine = Engine::default();
        let look = Look {
            label: "CI red".into(),
            accent: Some("red".into()),
            background: None,
        };
        engine.set_rules(vec![Rule {
            id: "r1".into(),
            status_id: "st1".into(),
            look: look.clone(),
            kind: RuleKind::Fact {
                name: "git.checks_failed".into(),
            },
            enabled: true,
        }]);
        let facts: HashMap<Id, PaneFacts> = HashMap::new();
        let mut p = Provided::default();

        assert!(!engine.facts(&p.holding(&ws)), "no fact, no status");
        assert!(p.set_fact(
            "git",
            &Target::Tab(tab.id.clone()),
            "checks_failed",
            Some(Value::Bool(true))
        ));
        assert!(engine.facts(&p.holding(&ws)));
        let shown = status::compute(&ws, &facts, &engine);
        assert_eq!(shown[&side.id].source, Source::Rule);
        assert_eq!(shown[&side.id].look.as_ref(), Some(&look));
        assert_eq!(shown[&tab.id].look.as_ref(), Some(&look));
        assert!(!engine.facts(&p.holding(&ws)), "a held fact sets it once");

        // Another plugin's fact of the same short name does not count.
        p.set_fact(
            "git",
            &Target::Tab(tab.id.clone()),
            "checks_failed",
            Some(Value::Bool(false)),
        );
        p.set_fact(
            "ci",
            &Target::Pane(pane),
            "checks_failed",
            Some(Value::Bool(true)),
        );
        assert!(engine.facts(&p.holding(&ws)), "a false fact clears it");
        assert_eq!(
            status::compute(&ws, &facts, &engine)[&side.id].source,
            Source::Auto
        );

        p.set_fact(
            "git",
            &Target::SideTab(side.id.clone()),
            "checks_failed",
            Some(Value::from(2)),
        );
        assert!(
            engine.facts(&p.holding(&ws)),
            "a row's fact holds for its panes"
        );
        assert!(p.forget_plugin("git"));
        assert!(engine.facts(&p.holding(&ws)), "a stopped plugin's facts go");
        assert_eq!(
            status::compute(&ws, &facts, &engine)[&side.id].source,
            Source::Auto
        );
    }

    #[test]
    fn a_saved_variable_wins_over_a_plugin_variable() {
        let db = Db::open_in_memory().unwrap();
        let c = core();
        let ws = c.workspace().clone();
        persist::save(
            &db,
            &persist::Saved {
                workspace: ws.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let side = &ws.windows[0].side_tabs[0];
        let pane = side.tabs[0].layout.terminals()[0].clone();
        context::put_saved(&db, &side.id, "branch", "mine", "typed", None, false).unwrap();
        let mut p = Provided::default();
        p.set_variable("git", &Target::Pane(pane), "branch", Some("main".into()));
        p.set_variable(
            "git",
            &Target::SideTab(side.id.clone()),
            "repo",
            Some("shop-api".into()),
        );
        p.set_variable(
            "other",
            &Target::Tab(side.tabs[0].id.clone()),
            "repo",
            Some("x".into()),
        );

        let saved = context::saved_variables(&db, &side.id).unwrap();
        let all = p.with_variables(&ws, &side.id, saved);
        let get = |n: &str| all.iter().find(|v| v.name == n).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            get("branch").value.as_deref(),
            Some("mine"),
            "the saved one wins"
        );
        assert!(!is_provided(get("branch")));
        assert_eq!(
            get("repo").value.as_deref(),
            Some("shop-api"),
            "the row's own comes first"
        );
        assert!(is_provided(get("repo")));
        assert_eq!(get("repo").source.as_deref(), Some("plugin:git"));
    }
}
