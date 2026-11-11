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
    /// The shared git folder has no working tree of its own (`core.bare`). A folder named
    /// `*.git` alone does not say so: `--separate-git-dir ../shop.git` is a normal repository.
    pub common_bare: bool,
    pub head: Head,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Changed, staged and untracked files; an untracked folder counts once.
    pub changed: u32,
    /// Files with an unresolved conflict.
    pub conflicted: u32,
    pub op: Option<Op>,
}

impl RepoFacts {
    /// The repository's name: its main working tree's folder, or for a bare repository `shop`
    /// from `shop.git` and the parent folder of a `.bare` one (polish-git-plugin D5).
    pub fn repo_name(&self) -> String {
        let name = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        match self.common_dir.file_name().and_then(|n| n.to_str()) {
            Some(".git" | ".bare") => self
                .common_dir
                .parent()
                .map(name)
                .unwrap_or_else(|| name(&self.root)),
            Some(n) if self.common_bare && n.len() > 4 && n.ends_with(".git") => {
                n[..n.len() - 4].to_string()
            }
            _ => name(&self.root),
        }
    }

    /// Where git runs for the repository's worktree list and removes, and the folder new
    /// worktrees go beside: the main working tree, or the shared git folder of a bare
    /// repository.
    pub fn main_root(&self) -> PathBuf {
        match self.common_dir.file_name().and_then(|n| n.to_str()) {
            Some(".git") => self
                .common_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.root.clone()),
            Some(n) if n == ".bare" || (self.common_bare && n.len() > 4 && n.ends_with(".git")) => {
                self.common_dir.clone()
            }
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
    /// Deleted, staged or not: the file is gone from the working tree.
    pub deleted: bool,
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

#[cfg(test)]
mod bare_tests {
    use super::*;

    fn facts(root: &str, common: &str) -> RepoFacts {
        RepoFacts {
            root: root.into(),
            git_dir: common.into(),
            common_dir: common.into(),
            common_bare: common.ends_with(".git") && !common.ends_with("/.git"),
            head: Head::Branch("main".into()),
            upstream: None,
            ahead: 0,
            behind: 0,
            changed: 0,
            conflicted: 0,
            op: None,
        }
    }

    #[test]
    fn bare_layouts_name_the_repository_and_its_main_folder() {
        let plain = facts("/code/shop-fix", "/code/shop/.git");
        assert_eq!(plain.repo_name(), "shop");
        assert_eq!(plain.main_root(), PathBuf::from("/code/shop"));
        let dot_git = facts("/code/shop-fix", "/code/shop.git");
        assert_eq!(dot_git.repo_name(), "shop");
        assert_eq!(dot_git.main_root(), PathBuf::from("/code/shop.git"));
        let dot_bare = facts("/code/shop/fix", "/code/shop/.bare");
        assert_eq!(dot_bare.repo_name(), "shop");
        assert_eq!(dot_bare.main_root(), PathBuf::from("/code/shop/.bare"));
        // A separate git folder named `*.git` of a normal repository keeps the working tree.
        let separate = RepoFacts {
            common_bare: false,
            ..facts("/code/foo", "/code/foo.git")
        };
        assert_eq!(separate.repo_name(), "foo");
        assert_eq!(separate.main_root(), PathBuf::from("/code/foo"));
        // A separate git folder with another name keeps the working tree.
        let other = facts("/code/shop", "/gitdirs/shop-data");
        assert_eq!(other.repo_name(), "shop");
        assert_eq!(other.main_root(), PathBuf::from("/code/shop"));
    }

    #[test]
    fn a_bare_repository_puts_worktrees_beside_it() {
        let f = facts("/code/shop-fix", "/code/shop.git");
        let at = crate::worktree::path_for(&f.main_root(), &f.repo_name(), "next", None);
        assert_eq!(at, PathBuf::from("/code/shop-next"));
    }
}
