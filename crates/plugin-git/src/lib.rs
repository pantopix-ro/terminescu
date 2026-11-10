//! Terminescu's git plugin (add-git-plugin). It reads the repository around each pane through a
//! version-control provider (Strategy, D1) wrapped in timing, caching and coalescing layers
//! (Decorator, D2), and never takes git's index lock.

pub mod cli;
pub mod forge;
pub mod layers;
pub mod plugin;
/// The timed runner moved to the plugin API, which the Claude plugin uses too.
pub use terminescu_plugin_api::process;
pub mod vcs;
pub mod worktree;

pub use cli::GitCli;
pub use forge::{Checks, Forge, ForgeError, GhCli, PullRequest};
pub use layers::{CachedVcs, CoalescingVcs, TimedVcs};
pub use plugin::GitPlugin;
pub use vcs::{Change, DiffBase, Head, Op, RepoFacts, Vcs, VcsError, Worktree};
