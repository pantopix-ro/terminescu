//! Session and secret variables, per side tab, in memory only (D21, D35, D36).
//!
//! Values never touch the disk. Each value is wiped from memory when it is replaced, unset or
//! expires; the daemon sweeps expired ones a few times a second, so a TTL holds even when no
//! shell asks. Copies made to answer a request live only until that answer is written.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};
use terminescu_protocol::{Var, VarMode};
use zeroize::Zeroizing;

struct Entry {
    value: Zeroizing<String>,
    mode: VarMode,
    expires: Option<Instant>,
}

#[derive(Default)]
struct SideVars {
    generation: u64,
    vars: BTreeMap<String, Entry>,
}

impl SideVars {
    fn has_expired(&self, now: Instant) -> bool {
        self.vars
            .values()
            .any(|e| e.expires.is_some_and(|t| t <= now))
    }
}

#[derive(Default)]
pub struct Vars {
    by_side: HashMap<String, SideVars>,
    /// One counter for all side tabs, so a pane that moves to another side tab never sees the
    /// generation it already loaded.
    next_generation: u64,
    /// Side tabs whose variables expired on a read, for the next sweep to report.
    expired: Vec<String>,
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
                value: Zeroizing::new(value),
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

    /// Wipes every variable of a side tab. Returns whether it had any.
    pub fn drop_side(&mut self, side: &str) -> bool {
        self.expired.retain(|s| s != side);
        self.by_side
            .remove(side)
            .is_some_and(|s| !s.vars.is_empty())
    }

    /// Moves variables to another side tab with their value, mode and expiry. A move onto a
    /// name the target has replaces it; a name the source lacks is skipped. Returns whether
    /// anything moved.
    pub fn move_vars(&mut self, from: &str, to: &str, moves: &[(String, String)]) -> bool {
        if from == to {
            return false;
        }
        let taken: Vec<(String, Entry)> = match self.by_side.get_mut(from) {
            Some(s) => moves
                .iter()
                .filter_map(|(a, b)| s.vars.remove(a).map(|e| (b.clone(), e)))
                .collect(),
            None => return false,
        };
        if taken.is_empty() {
            return false;
        }
        self.bump(from);
        let target = self.bump(to);
        for (name, e) in taken {
            target.vars.insert(name, e);
        }
        true
    }

    /// Drops every expired variable. Returns the side tabs that lost one since the last sweep,
    /// including those a read expired.
    pub fn sweep(&mut self) -> Vec<String> {
        let now = Instant::now();
        let sides: Vec<String> = self
            .by_side
            .iter()
            .filter(|(_, s)| s.has_expired(now))
            .map(|(side, _)| side.clone())
            .collect();
        for side in &sides {
            self.expire(side, now);
        }
        let mut changed = std::mem::take(&mut self.expired);
        changed.sort();
        changed.dedup();
        changed
    }

    fn expire(&mut self, side: &str, now: Instant) {
        self.bump(side)
            .vars
            .retain(|_, e| e.expires.is_none_or(|t| t > now));
        self.expired.push(side.into());
    }

    /// The side tab's variables for the app's context panel: secret values never leave the
    /// daemon this way.
    pub fn list_masked(&mut self, side: &str) -> (u64, Vec<Var>) {
        let (generation, mut vars) = self.get(side, None);
        for v in &mut vars {
            if v.mode == VarMode::Secret {
                v.value = String::new();
            }
        }
        (generation, vars)
    }

    /// The side tab's generation and variables, or just `name`. Drops expired ones first, so a
    /// read between two sweeps never returns one.
    pub fn get(&mut self, side: &str, name: Option<&str>) -> (u64, Vec<Var>) {
        let now = Instant::now();
        if self.by_side.get(side).is_some_and(|s| s.has_expired(now)) {
            self.expire(side, now);
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
                value: e.value.to_string(),
                mode: e.mode,
                expires_in_ms: e
                    .expires
                    .map(|t| t.saturating_duration_since(now).as_millis() as u64),
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
    fn move_keeps_mode_and_expiry_and_renames() {
        let mut v = Vars::default();
        v.set(
            "s1",
            "branch",
            "feature/PROJ-477".into(),
            VarMode::Session,
            None,
        );
        v.set(
            "s1",
            "API_TOKEN",
            "tok-example".into(),
            VarMode::Secret,
            Some(Duration::from_secs(60)),
        );
        v.set(
            "s2",
            "branch",
            "feature/PROJ-482".into(),
            VarMode::Session,
            None,
        );
        let moves = [
            ("branch".to_string(), "branch_477".to_string()),
            ("API_TOKEN".to_string(), "API_TOKEN".to_string()),
            ("gone".to_string(), "gone".to_string()),
        ];
        assert!(v.move_vars("s1", "s2", &moves));
        assert!(v.get("s1", None).1.is_empty());
        let (_, vars) = v.get("s2", None);
        let names: Vec<_> = vars.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["API_TOKEN", "branch", "branch_477"]);
        let token = vars.iter().find(|v| v.name == "API_TOKEN").unwrap();
        assert_eq!(token.mode, VarMode::Secret);
        assert!(token.expires_in_ms.is_some());
        assert!(!v.move_vars("s1", "s2", &moves), "nothing left to move");
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

    #[test]
    fn a_sweep_drops_expired_variables_without_a_read() {
        let mut v = Vars::default();
        v.set("s1", "K", "y".into(), VarMode::Session, None);
        v.set(
            "s2",
            "T",
            "x".into(),
            VarMode::Secret,
            Some(Duration::from_millis(20)),
        );
        assert!(v.sweep().is_empty(), "nothing has expired yet");
        let g1 = v.by_side["s1"].generation;
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(v.sweep(), ["s2"]);
        assert!(v.by_side["s2"].vars.is_empty());
        assert_eq!(
            v.by_side["s1"].generation, g1,
            "other side tabs keep theirs"
        );
        assert!(v.sweep().is_empty());
    }

    #[test]
    fn a_read_that_expires_is_reported_by_the_next_sweep() {
        let mut v = Vars::default();
        v.set(
            "s1",
            "T",
            "x".into(),
            VarMode::Secret,
            Some(Duration::from_millis(20)),
        );
        let (_, listed) = v.list_masked("s1");
        assert_eq!(
            listed[0].value, "",
            "a secret value never leaves through a list"
        );
        assert!(listed[0].expires_in_ms.is_some_and(|ms| ms <= 20));
        std::thread::sleep(Duration::from_millis(30));
        assert!(v.get("s1", None).1.is_empty());
        assert_eq!(v.sweep(), ["s1"]);
        v.set("s1", "K", "y".into(), VarMode::Session, None);
        assert!(v.drop_side("s1"));
        assert!(!v.drop_side("s1"));
        assert_eq!(v.get("s1", None), (0, vec![]));
    }
}
