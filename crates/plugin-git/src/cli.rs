//! `GitCli`: the person's `git`, read-only unless asked (add-git-plugin D1). Every call runs
//! with `--no-optional-locks`, so `status` never writes the index or takes its lock.

use crate::process::{self, Output, RunError};
use crate::vcs::{Change, DiffBase, Head, Op, RepoFacts, Vcs, VcsError, Worktree};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Untracked files longer than this are not counted line by line.
const COUNT_LINES_UP_TO: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct GitCli {
    program: PathBuf,
    /// A read still running after this is killed.
    limit: Duration,
    /// The same for a change (worktree add, remove), which a kill would leave half done.
    change_limit: Duration,
    env: Vec<(OsString, OsString)>,
}

/// How long a change such as `worktree add` may run. The plugin's call budget covers the
/// longest call: two changes and a `gh pr checkout` of the same length.
pub const CHANGE_LIMIT: Duration = Duration::from_secs(100);

impl Default for GitCli {
    fn default() -> Self {
        Self::new("git")
    }
}

impl GitCli {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            limit: Duration::from_secs(1),
            change_limit: CHANGE_LIMIT,
            env: Vec::new(),
        }
    }

    /// Kills a read after `limit`.
    pub fn with_limit(mut self, limit: Duration) -> Self {
        self.limit = limit;
        self
    }

    /// Kills a change such as `worktree add` after `limit`; [`CHANGE_LIMIT`] unless set.
    pub fn with_change_limit(mut self, limit: Duration) -> Self {
        self.change_limit = limit;
        self
    }

    /// Extra environment for every call, as tests use to leave out the person's config.
    pub fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    /// `git --version`, checked against the 2.30 the porcelain formats need.
    pub fn version(&self) -> Result<String, VcsError> {
        let out = self.run(Path::new("/"), &["--version"], self.limit)?;
        let text = String::from_utf8_lossy(&out.stdout);
        let version = text
            .split_whitespace()
            .nth(2)
            .unwrap_or_default()
            .to_string();
        let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        if (major, minor) < (2, 30) {
            return Err(VcsError::TooOld(version));
        }
        Ok(version)
    }

    /// Runs git in `dir` and kills it after `limit`.
    pub fn run(&self, dir: &Path, args: &[&str], limit: Duration) -> Result<Output, VcsError> {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-C")
            .arg(dir)
            .args(["--no-optional-locks", "-c", "core.quotepath=off"])
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            // A repository chosen by the shell around the app must not leak in.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_COMMON_DIR");
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        process::run(&mut cmd, limit).map_err(|e| match e {
            RunError::NotFound => VcsError::NoGit,
            RunError::Timeout => VcsError::Timeout,
            RunError::Io(e) => VcsError::Failed(e),
        })
    }

    fn read(&self, dir: &Path, args: &[&str]) -> Result<Output, VcsError> {
        self.run(dir, args, self.limit)
    }

    fn change(&self, dir: &Path, args: &[&str]) -> Result<(), VcsError> {
        let out = self.run(dir, args, self.change_limit)?;
        if out.ok {
            Ok(())
        } else {
            Err(VcsError::Failed(out.stderr))
        }
    }
}

impl Vcs for GitCli {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError> {
        let where_ = self.read(
            dir,
            &[
                "rev-parse",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ],
        )?;
        if !where_.ok {
            // Outside a repository, or inside a git folder with no working tree.
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&where_.stdout);
        let mut lines = text.lines();
        let (Some(root), Some(git_dir), Some(common)) = (lines.next(), lines.next(), lines.next())
        else {
            return Ok(None);
        };
        let (root, git_dir) = (PathBuf::from(root), PathBuf::from(git_dir));
        // Relative to `dir` when it is the main working tree's `.git`. Git hands out the other
        // two with symlinks resolved, so resolve this one too.
        let common_dir =
            std::fs::canonicalize(dir.join(common)).unwrap_or_else(|_| clean(&dir.join(common)));
        let status = self.read(
            &root,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                // An untracked folder counts once: `all` walks a stray build folder
                // and runs past the read limit. `changes` lists each file.
                "--untracked-files=normal",
            ],
        )?;
        if !status.ok {
            return Err(VcsError::Failed(status.stderr));
        }
        let parsed = parse_status(&status.stdout);
        Ok(Some(RepoFacts {
            op: op_in(&git_dir),
            root,
            git_dir,
            common_dir,
            head: parsed.head,
            upstream: parsed.upstream,
            ahead: parsed.ahead,
            behind: parsed.behind,
            changed: parsed.entries.len() as u32,
            conflicted: parsed.entries.iter().filter(|c| c.conflicted).count() as u32,
        }))
    }

    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError> {
        let status = self.read(
            root,
            &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        )?;
        if !status.ok {
            return Err(VcsError::Failed(status.stderr));
        }
        let mut changes = parse_status(&status.stdout).entries;
        // Lines against HEAD, staged and unstaged together. An unborn branch has no HEAD.
        let numstat = self.read(root, &["diff", "--numstat", "-z", "HEAD", "--"])?;
        if numstat.ok {
            let counts = parse_numstat(&numstat.stdout);
            for c in &mut changes {
                if let Some(&(added, removed)) = counts.get(&c.path) {
                    c.binary = added.is_none();
                    (c.added, c.removed) = (added, removed);
                }
            }
        }
        for c in changes.iter_mut().filter(|c| c.untracked) {
            let file = root.join(&c.path);
            // Not through a link, and never a device or a pipe.
            if std::fs::symlink_metadata(&file)
                .is_ok_and(|m| m.is_file() && m.len() <= COUNT_LINES_UP_TO)
                && let Ok(bytes) = std::fs::read(&file)
            {
                if bytes.contains(&0) {
                    c.binary = true;
                } else {
                    let lines = bytes.iter().filter(|&&b| b == b'\n').count()
                        + usize::from(bytes.last().is_some_and(|&b| b != b'\n'));
                    (c.added, c.removed) = (Some(lines as u32), Some(0));
                }
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }

    fn base_text(
        &self,
        root: &Path,
        file: &Path,
        base: DiffBase,
    ) -> Result<Option<Vec<u8>>, VcsError> {
        let rel = inside(file)?;
        let spec = match base {
            DiffBase::Head => format!("HEAD:{rel}"),
            DiffBase::Index => format!(":{rel}"),
        };
        // Absent, or a submodule's commit rather than a file.
        let kind = self.read(root, &["cat-file", "-t", &spec])?;
        if !kind.ok || kind.stdout.trim_ascii() != b"blob" {
            return Ok(None);
        }
        let out = self.read(root, &["cat-file", "blob", &spec])?;
        if !out.ok {
            return Err(VcsError::Failed(out.stderr));
        }
        Ok(Some(out.stdout))
    }

    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError> {
        free(at)?;
        name(branch)?;
        name(base)?;
        let at = at.to_string_lossy();
        self.change(root, &["worktree", "add", "-b", branch, "--", &at, base])
    }

    fn worktree_checkout(&self, root: &Path, branch: &str, at: &Path) -> Result<(), VcsError> {
        free(at)?;
        name(branch)?;
        let at = at.to_string_lossy();
        self.change(root, &["worktree", "add", "--", &at, branch])
    }

    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError> {
        let out = self.read(root, &["worktree", "list", "--porcelain"])?;
        if !out.ok {
            return Err(VcsError::Failed(out.stderr));
        }
        Ok(parse_worktrees(&String::from_utf8_lossy(&out.stdout)))
    }

    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError> {
        let at = at.to_string_lossy();
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.extend(["--", &at]);
        let out = self.run(root, &args, self.change_limit)?;
        if out.ok {
            return Ok(());
        }
        // "contains modified or untracked files, use --force to delete it"
        if !force && out.stderr.contains("--force") {
            return Err(VcsError::HasChanges);
        }
        Err(VcsError::Failed(out.stderr))
    }
}

/// An absolute path with nothing there yet. Git would read a relative one against the root.
fn free(at: &Path) -> Result<(), VcsError> {
    if !at.is_absolute() {
        return Err(VcsError::Failed(format!(
            "{} is not an absolute path",
            at.display()
        )));
    }
    if at.symlink_metadata().is_ok() {
        return Err(VcsError::Exists(at.display().to_string()));
    }
    Ok(())
}

/// A branch or base git cannot read as an option.
fn name(n: &str) -> Result<(), VcsError> {
    if n.is_empty() || n.starts_with('-') {
        return Err(VcsError::Failed(format!("{n:?} is not a branch name")));
    }
    Ok(())
}

/// A path relative to the root that stays inside it.
fn inside(file: &Path) -> Result<String, VcsError> {
    let ok = !file.as_os_str().is_empty()
        && file.components().all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(VcsError::Failed(format!(
            "{} is not a path inside the repository",
            file.display()
        )));
    }
    Ok(file.to_string_lossy().into_owned())
}

/// `a/b/../c` as `a/c`, without touching the disk.
fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn op_in(git_dir: &Path) -> Option<Op> {
    if git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir() {
        Some(Op::Rebase)
    } else if git_dir.join("MERGE_HEAD").is_file() {
        Some(Op::Merge)
    } else if git_dir.join("CHERRY_PICK_HEAD").is_file() {
        Some(Op::CherryPick)
    } else {
        None
    }
}

pub struct Status {
    pub head: Head,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<Change>,
}

/// `git status --porcelain=v2 --branch -z`.
pub fn parse_status(bytes: &[u8]) -> Status {
    let mut oid = String::new();
    let mut head = None;
    let mut s = Status {
        head: Head::Detached(String::new()),
        upstream: None,
        ahead: 0,
        behind: 0,
        entries: Vec::new(),
    };
    let mut records = bytes
        .split(|&b| b == 0)
        .map(|r| String::from_utf8_lossy(r).into_owned());
    while let Some(rec) = records.next() {
        let change = |path: &str, xy: &str| {
            let (x, y) = (xy.chars().next(), xy.chars().nth(1));
            Change {
                path: path.to_string(),
                from: None,
                staged: x.is_some_and(|c| c != '.'),
                unstaged: y.is_some_and(|c| c != '.'),
                untracked: false,
                conflicted: false,
                added: None,
                removed: None,
                binary: false,
            }
        };
        if let Some(h) = rec.strip_prefix("# ") {
            let (key, value) = h.split_once(' ').unwrap_or((h, ""));
            match key {
                "branch.oid" => oid = value.to_string(),
                "branch.head" => head = Some(value.to_string()),
                "branch.upstream" => s.upstream = Some(value.to_string()),
                "branch.ab" => {
                    for part in value.split(' ') {
                        if let Some(n) = part.strip_prefix('+') {
                            s.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            s.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
        } else if let Some(rest) = rec.strip_prefix("1 ") {
            let f: Vec<&str> = rest.splitn(8, ' ').collect();
            if let (Some(xy), Some(path)) = (f.first(), f.get(7)) {
                s.entries.push(change(path, xy));
            }
        } else if let Some(rest) = rec.strip_prefix("2 ") {
            let f: Vec<&str> = rest.splitn(9, ' ').collect();
            if let (Some(xy), Some(path)) = (f.first(), f.get(8)) {
                let mut c = change(path, xy);
                c.from = records.next();
                s.entries.push(c);
            }
        } else if let Some(rest) = rec.strip_prefix("u ") {
            let f: Vec<&str> = rest.splitn(10, ' ').collect();
            if let (Some(xy), Some(path)) = (f.first(), f.get(9)) {
                let mut c = change(path, xy);
                c.conflicted = true;
                s.entries.push(c);
            }
        } else if let Some(path) = rec.strip_prefix("? ") {
            let mut c = change(path, "..");
            c.untracked = true;
            s.entries.push(c);
        }
    }
    s.head = match head.as_deref() {
        Some("(detached)") | None => Head::Detached(oid.chars().take(7).collect()),
        Some(b) if oid == "(initial)" => Head::Unborn(b.to_string()),
        Some(b) => Head::Branch(b.to_string()),
    };
    s
}

/// `git diff --numstat -z`: lines added and removed by the new path; `None` for a binary file.
pub fn parse_numstat(bytes: &[u8]) -> HashMap<String, (Option<u32>, Option<u32>)> {
    let mut out = HashMap::new();
    let mut records = bytes
        .split(|&b| b == 0)
        .map(|r| String::from_utf8_lossy(r).into_owned());
    while let Some(rec) = records.next() {
        let mut f = rec.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let path = if path.is_empty() {
            // A rename: the old and the new path follow as their own records.
            let _from = records.next();
            match records.next() {
                Some(p) => p,
                None => continue,
            }
        } else {
            path.to_string()
        };
        out.insert(path, (a.parse().ok(), r.parse().ok()));
    }
    out
}

/// `git worktree list --porcelain`: blocks of lines, one per worktree, the main one first.
pub fn parse_worktrees(text: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    for block in text.split("\n\n") {
        let mut w = Worktree {
            path: PathBuf::new(),
            branch: None,
            head: None,
            bare: false,
            main: out.is_empty(),
        };
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => w.path = PathBuf::from(value),
                "HEAD" => w.head = Some(value.to_string()),
                "branch" => {
                    w.branch = Some(value.strip_prefix("refs/heads/").unwrap_or(value).into())
                }
                "bare" => w.bare = true,
                _ => {}
            }
        }
        if !w.path.as_os_str().is_empty() {
            out.push(w);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_branch_counts_and_every_entry_kind() {
        let raw = b"# branch.oid 1234567890abcdef\0# branch.head feature/PROJ-482-retry\0\
# branch.upstream origin/feature/PROJ-482-retry\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb src/retry file.ts\0\
2 R. N... 100644 100644 100644 aaa bbb R100 src/new.ts\0src/old.ts\0\
u UU N... 100644 100644 100644 100644 aaa bbb ccc src/conflict.ts\0\
? notes.md\0";
        let s = parse_status(raw);
        assert_eq!(s.head, Head::Branch("feature/PROJ-482-retry".into()));
        assert_eq!(s.upstream.as_deref(), Some("origin/feature/PROJ-482-retry"));
        assert_eq!((s.ahead, s.behind), (2, 1));
        let paths: Vec<_> = s.entries.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "src/retry file.ts",
                "src/new.ts",
                "src/conflict.ts",
                "notes.md"
            ]
        );
        assert!(s.entries[0].unstaged && !s.entries[0].staged);
        assert_eq!(s.entries[1].from.as_deref(), Some("src/old.ts"));
        assert!(s.entries[1].staged);
        assert!(s.entries[2].conflicted);
        assert!(s.entries[3].untracked);
    }

    #[test]
    fn detached_and_unborn_heads() {
        let d = parse_status(b"# branch.oid 1234567890abc\0# branch.head (detached)\0");
        assert_eq!(d.head, Head::Detached("1234567".into()));
        let u = parse_status(b"# branch.oid (initial)\0# branch.head main\0");
        assert_eq!(u.head, Head::Unborn("main".into()));
    }

    #[test]
    fn numstat_reads_renames_and_binaries() {
        let raw = b"12\t3\tsrc/retry.ts\0-\t-\tlogo.png\0" as &[u8];
        let rename = b"1\t0\t\0src/old.ts\0src/new.ts\0" as &[u8];
        let n = parse_numstat(&[raw, rename].concat());
        assert_eq!(n["src/retry.ts"], (Some(12), Some(3)));
        assert_eq!(n["logo.png"], (None, None));
        assert_eq!(n["src/new.ts"], (Some(1), Some(0)));
    }

    #[test]
    fn worktree_list_blocks() {
        let text = "worktree /code/shop-api\nHEAD abc\nbranch refs/heads/develop\n\n\
worktree /code/shop-api-PROJ-490\nHEAD def\nbranch refs/heads/PROJ-490\n\n\
worktree /code/shop-api-detached\nHEAD 123\ndetached\n";
        let w = parse_worktrees(text);
        assert_eq!(w.len(), 3);
        assert!(w[0].main && !w[1].main);
        assert_eq!(w[1].branch.as_deref(), Some("PROJ-490"));
        assert_eq!(w[2].branch, None);
    }

    #[test]
    fn a_file_must_stay_inside_the_repository() {
        assert!(inside(Path::new("src/a.ts")).is_ok());
        assert!(inside(Path::new("../a.ts")).is_err());
        assert!(inside(Path::new("/etc/passwd")).is_err());
        assert!(inside(Path::new("")).is_err());
    }
}
