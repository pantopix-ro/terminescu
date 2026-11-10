//! Terminescu's git plugin (add-git-plugin). It reads the repository around each pane through a
//! version-control provider (Strategy, D1) wrapped in timing, caching and coalescing layers
//! (Decorator, D2), and never takes git's index lock.

pub mod cli;
pub mod layers;
pub mod vcs;

pub use cli::GitCli;
pub use layers::{CachedVcs, CoalescingVcs, TimedVcs};
pub use vcs::{Change, DiffBase, Head, Op, RepoFacts, Vcs, VcsError, Worktree};
