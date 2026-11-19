//! Decorations (D6): small things a plugin shows on a row, a tab or a pane. They live in
//! memory only; a plugin rebuilds them on `AppLaunched`.

use serde::{Deserialize, Serialize};

/// What a decoration, a fact or a variable is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Target {
    SideTab(String),
    Tab(String),
    Pane(String),
}

/// Where in the target a decoration shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slot {
    Chip,
    Subtitle,
    Badge,
    Meter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Neutral,
    Good,
    Warn,
    Bad,
}

/// A small mark on a link row in the context panel, as a PR's state (refine-context-links D9).
/// It lives in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkBadge {
    pub text: String,
    pub tone: Tone,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoration {
    pub target: Target,
    pub slot: Slot,
    /// Unique per plugin, target and slot: a second set with the same key replaces the first.
    pub key: String,
    pub text: String,
    pub tone: Tone,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}
