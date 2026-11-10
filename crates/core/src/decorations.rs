//! Plugin decorations (add-plugin-architecture D6). In memory only: they change every few
//! seconds, and a plugin rebuilds its own on `AppLaunched`.

use serde::Serialize;
use std::collections::BTreeMap;
use terminescu_plugin_api::{Decoration, Slot, Target};

/// A decoration with the plugin that set it, as the UI gets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    pub plugin: String,
    #[serde(flatten)]
    pub decoration: Decoration,
}

type Key = (String, String, Slot, String);

fn key_of(plugin: &str, d: &Decoration) -> Key {
    let target = serde_json::to_string(&d.target).unwrap_or_default();
    (plugin.to_string(), target, d.slot, d.key.clone())
}

#[derive(Debug, Default)]
pub struct Decorations {
    shown: BTreeMap<Key, Shown>,
}

impl Decorations {
    /// Adds or replaces by plugin, target, slot and key. True when anything changed.
    pub fn set(&mut self, plugin: &str, decoration: Decoration) -> bool {
        let next = Shown {
            plugin: plugin.to_string(),
            decoration,
        };
        let key = key_of(plugin, &next.decoration);
        if self.shown.get(&key) == Some(&next) {
            return false;
        }
        self.shown.insert(key, next);
        true
    }

    /// Clears a plugin's decorations, on one target or on all. True when anything changed.
    pub fn clear(&mut self, plugin: &str, target: Option<&Target>) -> bool {
        let before = self.shown.len();
        self.shown
            .retain(|_, s| s.plugin != plugin || target.is_some_and(|t| *t != s.decoration.target));
        self.shown.len() != before
    }

    /// Clears every plugin's decorations on targets that are gone.
    pub fn forget(&mut self, gone: &Target) -> bool {
        let before = self.shown.len();
        self.shown.retain(|_, s| s.decoration.target != *gone);
        self.shown.len() != before
    }

    /// Every decoration, in plugin, target, slot and key order.
    pub fn list(&self) -> Vec<Shown> {
        self.shown.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminescu_plugin_api::Tone;

    fn chip(target: Target, key: &str, text: &str) -> Decoration {
        Decoration {
            target,
            slot: Slot::Chip,
            key: key.into(),
            text: text.into(),
            tone: Tone::Neutral,
            tip: None,
        }
    }

    #[test]
    fn decorations_merge_by_key_and_clear_per_plugin() {
        let mut d = Decorations::default();
        let row = Target::SideTab("s1".into());
        let other = Target::SideTab("s2".into());
        assert!(d.set("git", chip(row.clone(), "branch", "main")));
        assert!(
            !d.set("git", chip(row.clone(), "branch", "main")),
            "same value, no change"
        );
        assert!(
            d.set("git", chip(row.clone(), "branch", "feature/x")),
            "same key replaces"
        );
        assert!(d.set("git", chip(other.clone(), "branch", "main")));
        assert!(
            d.set("claude", chip(row.clone(), "branch", "agent")),
            "keys are per plugin"
        );
        let mut subtitle = chip(row.clone(), "branch", "2 ahead");
        subtitle.slot = Slot::Subtitle;
        assert!(d.set("git", subtitle), "keys are per slot");
        assert_eq!(d.list().len(), 4);
        assert_eq!(
            d.list()
                .iter()
                .find(|s| s.plugin == "git"
                    && s.decoration.slot == Slot::Chip
                    && s.decoration.target == row)
                .map(|s| s.decoration.text.as_str()),
            Some("feature/x")
        );

        assert!(d.clear("git", Some(&other)));
        assert_eq!(d.list().len(), 3);
        assert!(d.clear("git", None));
        assert_eq!(d.list().len(), 1, "the other plugin keeps its own");
        assert_eq!(d.list()[0].plugin, "claude");
        assert!(!d.clear("git", None));
        assert!(d.forget(&row));
        assert!(d.list().is_empty());

        let json = serde_json::to_value(Shown {
            plugin: "git".into(),
            decoration: chip(row, "branch", "main"),
        })
        .unwrap();
        assert_eq!(json["plugin"], "git");
        assert_eq!(
            json["target"],
            serde_json::json!({"kind": "sideTab", "id": "s1"})
        );
        assert_eq!(json["slot"], "chip");
    }
}
