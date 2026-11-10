//! The version-control provider (Strategy, add-git-plugin D1): what the plugin asks of a
//! repository, whatever answers it.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Where HEAD points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "camelCase")]
pub enum Head {
    Branch(String),
    /// A detached HEAD, by its short commit.
    Detached(String),
    /// A branch with no commit yet.
    Unborn(String),
}

impl Head {
    /// The branch, when HEAD is on one.
    pub fn branch(&self) -> Option<&str> {
        match self {
            Head::Branch(b) | Head::Unborn(b) => Some(b),
            Head::Detached(_) => None,
        }
    }

    /// What a chip shows: the branch, or the short commit.
    pub fn label(&self) -> &str {
        match self {
            Head::Branch(b) | Head::Unborn(b) | Head::Detached(b) => b,
        }
    }
}

/// An operation git stopped in the middle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Merge,
    Rebase,
    CherryPick,
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Merge => "Merge",
            Op::Rebase => "Rebase",
            Op::CherryPick => "Cherry-pick",
        }
    }
}

/// One repository as a pane's cwd sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoFacts {
    /// The working tree's top folder.
    pub root: PathBuf,
    /// This working tree's git folder: `.git`, or `.git/worktrees/<name>` in a worktree.
    pub git_dir: PathBuf,
    /// The repository's shared git folder; the main working tree's `.git`.
    pub common_dir: PathBuf,
    pub head: Head,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Changed, staged and untracked files.
    pub changed: u32,
    /// Files with an unresolved conflict.
    pub conflicted: u32,
    pub op: Option<Op>,
}

impl RepoFacts {
    /// The repository's name: its main working tree's folder.
    pub fn repo_name(&self) -> String {
        let main = if self.common_dir.file_name().is_some_and(|n| n == ".git") {
            self.common_dir.parent().unwrap_or(&self.root)
        } else {
            &self.root
        };
        main.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The main working tree, where worktrees are made from.
    pub fn main_root(&self) -> PathBuf {
        match self.common_dir.file_name() {
            Some(n) if n == ".git" => self
                .common_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.root.clone()),
            _ => self.root.clone(),
        }
    }

    /// Pushed: on a branch with an upstream and nothing ahead of it.
    pub fn pushed(&self) -> bool {
        self.head.branch().is_some() && self.upstream.is_some() && self.ahead == 0
    }
}

/// One changed file, relative to the root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    /// The old path of a rename or copy.
    pub from: Option<String>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
    /// Lines added and removed against HEAD; `None` for a binary file or when unknown.
    pub added: Option<u32>,
    pub removed: Option<u32>,
    pub binary: bool,
}

/// The side a diff compares the working copy with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffBase {
    Head,
    /// The staged version.
    Index,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: PathBuf,
    /// `None` when detached or bare.
    pub branch: Option<String>,
    pub head: Option<String>,
    pub bare: bool,
    /// The main working tree, listed first by git.
    pub main: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VcsError {
    #[error("git is not installed")]
    NoGit,
    #[error("git {0} is older than 2.30")]
    TooOld(String),
    #[error("git took longer than its limit")]
    Timeout,
    #[error("the worktree has changes")]
    HasChanges,
    #[error("{0} already exists")]
    Exists(String),
    #[error("{0}")]
    Failed(String),
}

/// A version-control provider (Strategy, D1). Every path is absolute; a file in
/// [`Vcs::base_text`] is relative to `root`.
pub trait Vcs: Send + Sync {
    /// The facts of the repository around `dir`; `None` outside one.
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError>;
    /// Every changed file of the working tree at `root`, sorted by path.
    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError>;
    /// The file's bytes at `base`; `None` when it is not there.
    fn base_text(
        &self,
        root: &Path,
        file: &Path,
        base: DiffBase,
    ) -> Result<Option<Vec<u8>>, VcsError>;
    /// A new branch `branch` from `base`, checked out in a worktree at `at`.
    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError>;
    /// A worktree at `at` for the branch that exists already, such as a fetched pull request.
    fn worktree_checkout(&self, root: &Path, branch: &str, at: &Path) -> Result<(), VcsError>;
    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError>;
    /// Removes the worktree, never its branch. Without `force` one with changes stays.
    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError>;
    /// Drops what is known about the repository around `dir`, after a command may have
    /// changed it.
    fn forget(&self, _dir: &Path) {}
}
