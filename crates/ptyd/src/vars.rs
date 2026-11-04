//! Session and secret variables, per side tab, in memory only (D21, D35, D36).
//!
//! Task 2.2 adds zeroizing and a TTL sweep; until then an expired variable disappears the next
//! time anyone reads its side tab.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};
use terminescu_protocol::{Var, VarMode};

struct Entry {
    value: String,
    mode: VarMode,
    expires: Option<Instant>,
}

#[derive(Default)]
struct SideVars {
    generation: u64,
    vars: BTreeMap<String, Entry>,
}

#[derive(Default)]
pub struct Vars {
    by_side: HashMap<String, SideVars>,
    /// One counter for all side tabs, so a pane that moves to another side tab never sees the
    /// generation it already loaded.
    next_generation: u64,
}

impl Vars {
    fn bump(&mut self, side: &str) -> &mut SideVars {
        self.next_generation += 1;
        let g = self.next_generation;
        let s = self.by_side.entry(side.into()).or_default();
        s.generation = g;
        s
    }

    pub fn set(
        &mut self,
        side: &str,
        name: &str,
        value: String,
        mode: VarMode,
        ttl: Option<Duration>,
    ) {
        let expires = ttl.map(|t| Instant::now() + t);
        self.bump(side).vars.insert(
            name.into(),
            Entry {
                value,
                mode,
                expires,
            },
        );
    }

    pub fn unset(&mut self, side: &str, name: &str) {
        if self
            .by_side
            .get(side)
            .is_some_and(|s| s.vars.contains_key(name))
        {
            self.bump(side).vars.remove(name);
        }
    }

    /// The side tab's generation and variables, or just `name`. Drops expired ones first.
    pub fn get(&mut self, side: &str, name: Option<&str>) -> (u64, Vec<Var>) {
        let now = Instant::now();
        let expired = self
            .by_side
            .get(side)
            .is_some_and(|s| s.vars.values().any(|e| e.expires.is_some_and(|t| t <= now)));
        if expired {
            self.bump(side)
                .vars
                .retain(|_, e| e.expires.is_none_or(|t| t > now));
        }
        let Some(s) = self.by_side.get(side) else {
            return (0, Vec::new());
        };
        let vars = s
            .vars
            .iter()
            .filter(|(n, _)| name.is_none_or(|want| want == n.as_str()))
            .map(|(n, e)| Var {
                name: n.clone(),
                value: e.value.clone(),
                mode: e.mode,
            })
            .collect();
        (s.generation, vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_are_per_side_tab_and_bump_the_generation() {
        let mut v = Vars::default();
        assert_eq!(v.get("s1", None), (0, vec![]));
        v.set("s1", "A", "1".into(), VarMode::Session, None);
        v.set("s2", "A", "2".into(), VarMode::Secret, None);
        let (g1, vars) = v.get("s1", None);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value, "1");
        assert_eq!(v.get("s2", Some("A")).1[0].mode, VarMode::Secret);
        assert!(v.get("s2", Some("B")).1.is_empty());
        v.unset("s1", "A");
        let (g2, vars) = v.get("s1", None);
        assert!(g2 > g1 && vars.is_empty());
        v.unset("s1", "A");
        assert_eq!(v.get("s1", None).0, g2, "unsetting nothing changes nothing");
    }

    #[test]
    fn expired_variables_are_gone_on_read() {
        let mut v = Vars::default();
        v.set(
            "s1",
            "T",
            "x".into(),
            VarMode::Secret,
            Some(Duration::from_millis(20)),
        );
        v.set("s1", "K", "y".into(), VarMode::Session, None);
        let (g1, _) = v.get("s1", None);
        std::thread::sleep(Duration::from_millis(30));
        let (g2, vars) = v.get("s1", None);
        assert!(g2 > g1);
        assert_eq!(
            vars.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["K"]
        );
    }
}
