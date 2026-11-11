//! Baselines (add-claude-insights D1, D2): what a session's files held when it started, and
//! what changed since. In a git repository a snapshot of the working tree goes into
//! Terminescu's own object store; everywhere, the hook's first-touch copies win over it.
//!
//! The person's repository is never written: the snapshot runs `git add -A` and `git
//! write-tree` with `GIT_INDEX_FILE` and `GIT_OBJECT_DIRECTORY` inside the session's folder and
//! the repository's objects as an alternate, read-only.

use serde::{Deserialize, Serialize};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use terminescu_plugin_api::baseline::{self as store, Copy, READY, SIZE_CAP};
use terminescu_plugin_api::process::{self, RunError};

/// A snapshot that takes longer stops; the session keeps its first-touch copies (D2 risks).
pub const SNAPSHOT_LIMIT: Duration = Duration::from_secs(30);
/// A git read for a list or a base text.
const READ_LIMIT: Duration = Duration::from_secs(5);
/// Baselines go this long after their session ends (the spec's default).
pub const KEEP: Duration = Duration::from_secs(7 * 24 * 3600);
/// The whole store's cap, oldest ended sessions first (the spec's default).
pub const CAP_BYTES: u64 = 500 * 1024 * 1024;

const META: &str = "meta.json";

/// What the plugin keeps about a session's baseline, in `<session>/meta.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Where the session started.
    pub cwd: String,
    /// The repository's top folder, when the session started in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// Another session whose baseline this one uses: a resumed or cleared session in the same
    /// pane keeps the first one (D1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Seconds since the epoch.
    pub started: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<u64>,
    /// Why the snapshot did not finish, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_failed: Option<String>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The baseline store of one Terminescu support folder.
#[derive(Debug, Clone)]
pub struct Store {
    pub dir: PathBuf,
    pub git: PathBuf,
}

fn git_env(cmd: &mut Command) {
    cmd.env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .args(["-c", "core.quotepath=off"]);
}

fn failed(e: RunError) -> String {
    match e {
        RunError::NotFound => "git is not installed".into(),
        RunError::Timeout => "git took too long".into(),
        RunError::TooLarge => "git wrote too much".into(),
        RunError::Io(e) => e,
    }
}

/// One file of a session's changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    /// Absolute.
    pub path: String,
    /// Null for a binary file or one with no copy to count against.
    pub added: Option<u64>,
    pub removed: Option<u64>,
    /// The file did not exist when the session started.
    pub created: bool,
    /// The file is gone now.
    pub deleted: bool,
    /// Why there is nothing to compare with ("too large"), when there is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub none: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub files: Vec<Changed>,
    /// A git snapshot covers the session: Bash and formatter edits show.
    pub tree: bool,
    /// The folder the paths read from: the repository's top folder, else the session's cwd.
    pub base: String,
}

/// A file's text before the session, as the diff view reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseText {
    pub exists: bool,
    pub binary: bool,
    pub large: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl BaseText {
    fn of(bytes: Vec<u8>) -> Self {
        if bytes.len() as u64 > SIZE_CAP {
            return Self {
                exists: true,
                binary: false,
                large: true,
                text: None,
            };
        }
        match String::from_utf8(bytes) {
            Ok(text) if !text.contains('\0') => Self {
                exists: true,
                binary: false,
                large: false,
                text: Some(text),
            },
            _ => Self {
                exists: true,
                binary: true,
                large: false,
                text: None,
            },
        }
    }
    fn missing() -> Self {
        Self {
            exists: false,
            binary: false,
            large: false,
            text: Some(String::new()),
        }
    }
}

impl Store {
    pub fn new(support: &Path) -> Self {
        Self {
            dir: store::store(support),
            git: PathBuf::from("git"),
        }
    }

    fn session(&self, id: &str) -> Option<PathBuf> {
        store::session_dir(&self.dir, id)
    }

    fn run_git(
        &self,
        cwd: &Path,
        args: &[&str],
        env: &[(&str, &Path)],
        limit: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut cmd = Command::new(&self.git);
        git_env(&mut cmd);
        cmd.current_dir(cwd).args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = process::run(&mut cmd, limit).map_err(failed)?;
        if !out.ok {
            return Err(if out.stderr.is_empty() {
                format!("git {} failed", args[0])
            } else {
                out.stderr
            });
        }
        Ok(out.stdout)
    }

    pub fn meta(&self, id: &str) -> Option<Meta> {
        let bytes = std::fs::read(self.session(id)?.join(META)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn set_meta(&self, id: &str, meta: &Meta) -> std::io::Result<()> {
        let dir = self
            .session(id)
            .ok_or_else(|| std::io::Error::other("not a session id"))?;
        private_dir(&dir)?;
        let bytes = serde_json::to_vec(meta).map_err(std::io::Error::other)?;
        terminescu_plugin_api::atomic::write(&dir.join(META), &bytes)
    }

    /// Reads the session's meta now, lets `change` edit it, and writes it back. For writers that
    /// held a copy for a while (the snapshot thread): only the fields they change are theirs,
    /// so a `SessionEnd` that landed meanwhile is not lost.
    pub fn update_meta(&self, id: &str, change: impl FnOnce(&mut Meta)) -> std::io::Result<()> {
        let mut meta = self.meta(id).unwrap_or_default();
        change(&mut meta);
        self.set_meta(id, &meta)
    }

    /// The session whose baseline `id` uses.
    fn holder(&self, id: &str) -> String {
        self.meta(id)
            .and_then(|m| m.base)
            .unwrap_or_else(|| id.to_string())
    }

    /// The top folder and git folders of the repository holding `cwd`, if any.
    pub fn repo_of(&self, cwd: &Path) -> Option<Repo> {
        let out = self
            .run_git(
                cwd,
                &[
                    "rev-parse",
                    "--show-toplevel",
                    "--absolute-git-dir",
                    "--git-common-dir",
                ],
                &[],
                READ_LIMIT,
            )
            .ok()?;
        let text = String::from_utf8(out).ok()?;
        let mut lines = text.lines();
        let root = PathBuf::from(lines.next()?);
        let git_dir = PathBuf::from(lines.next()?);
        let common = PathBuf::from(lines.next()?);
        let common = if common.is_absolute() {
            common
        } else {
            cwd.join(common)
        };
        Some(Repo {
            root,
            git_dir,
            common,
        })
    }

    /// The working-tree snapshot (D2). Writes `ready` with the tree id when it is done.
    pub fn snapshot(&self, id: &str, repo: &Repo, limit: Duration) -> Result<String, String> {
        let dir = self.session(id).ok_or("not a session id")?;
        if is_home(
            &repo.root,
            std::env::var_os("HOME").as_deref().map(Path::new),
        ) {
            return Err("the repository is the home folder".into());
        }
        private_dir(&dir.join("objects")).map_err(|e| e.to_string())?;
        let index = dir.join("index");
        // A copy of the repository's index: files whose stat data still match are not hashed again.
        if let Err(e) = std::fs::copy(repo.git_dir.join("index"), &index)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e.to_string());
        }
        let tree = self.write_tree(&dir, &index, repo, limit)?;
        // The tree, then the root the snapshot covers: the hook reads the second line.
        let ready = format!("{tree}\n{}\n", repo.root.display());
        terminescu_plugin_api::atomic::write(&dir.join(READY), ready.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(tree)
    }

    fn write_tree(
        &self,
        dir: &Path,
        index: &Path,
        repo: &Repo,
        limit: Duration,
    ) -> Result<String, String> {
        let objects = dir.join("objects");
        let alternates = repo.common.join("objects");
        let env: [(&str, &Path); 3] = [
            ("GIT_INDEX_FILE", index),
            ("GIT_OBJECT_DIRECTORY", &objects),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", &alternates),
        ];
        let start = std::time::Instant::now();
        // Untracked files that look like secrets or are over the size cap stay out of the store.
        // They go in as exclude pathspecs, which add to the person's own ignore rules; tracked
        // files are already in the repository's objects.
        let spec = dir.join("pathspec");
        let skipped = self.write_pathspec(repo, &spec, &env, start, limit)?;
        let mut add = vec!["add", "-A"];
        let from;
        if skipped {
            from = format!("--pathspec-from-file={}", spec.display());
            add.extend([from.as_str(), "--pathspec-file-nul"]);
        } else {
            add.extend(["--", "."]);
        }
        self.run_git(
            &repo.root,
            &add,
            &env,
            limit
                .saturating_sub(start.elapsed())
                .max(Duration::from_millis(100)),
        )?;
        let left = limit
            .saturating_sub(start.elapsed())
            .max(Duration::from_millis(100));
        let out = self.run_git(&repo.root, &["write-tree"], &env, left)?;
        let tree = String::from_utf8_lossy(&out).trim().to_string();
        if tree.len() < 40 || !tree.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("git write-tree gave no tree".into());
        }
        Ok(tree)
    }

    /// Writes `.` and one `:(exclude,literal)` pathspec per untracked, not ignored file that
    /// [`is_secret_path`] names or that is over [`SIZE_CAP`]. False when there is none.
    fn write_pathspec(
        &self,
        repo: &Repo,
        spec: &Path,
        env: &[(&str, &Path)],
        start: std::time::Instant,
        limit: Duration,
    ) -> Result<bool, String> {
        let list = self.run_git(
            &repo.root,
            &["ls-files", "-z", "--others", "--exclude-standard"],
            env,
            limit,
        )?;
        let mut out: Vec<u8> = Vec::new();
        for rel in list.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            if start.elapsed() > limit {
                return Err("git took too long".into());
            }
            let path = repo.root.join(std::ffi::OsStr::from_bytes(rel));
            let big =
                std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && m.len() > SIZE_CAP);
            if big || terminescu_plugin_api::baseline::is_secret_path(&path) {
                out.extend_from_slice(b":(exclude,literal)");
                out.extend_from_slice(rel);
                out.push(0);
            }
        }
        if out.is_empty() {
            let _ = std::fs::remove_file(spec);
            return Ok(false);
        }
        out.splice(0..0, b".\0".iter().copied());
        terminescu_plugin_api::atomic::write(spec, &out).map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn ready_tree(&self, id: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.session(id)?.join(READY)).ok()?;
        let tree = text.lines().next().unwrap_or("").trim().to_string();
        (!tree.is_empty()).then_some(tree)
    }

    /// What changed since the session started (1.3): the snapshot's files against a fresh
    /// snapshot of now, then each first-touch copy against the file, which wins.
    pub fn changes(&self, id: &str) -> Result<Changes, String> {
        let holder = self.holder(id);
        let meta = self.meta(&holder).ok_or("no baseline for this session")?;
        let dir = self.session(&holder).ok_or("not a session id")?;
        let mut files: Vec<Changed> = Vec::new();
        let tree = self.ready_tree(&holder);
        let repo = meta
            .root
            .as_deref()
            .and_then(|r| self.repo_of(Path::new(r)));
        if let (Some(base), Some(repo)) = (&tree, &repo) {
            let index = dir.join("now-index");
            let _ = std::fs::copy(dir.join("index"), &index);
            let now = self.write_tree(&dir, &index, repo, READ_LIMIT)?;
            let objects = dir.join("objects");
            let alternates = repo.common.join("objects");
            let env: [(&str, &Path); 2] = [
                ("GIT_OBJECT_DIRECTORY", &objects),
                ("GIT_ALTERNATE_OBJECT_DIRECTORIES", &alternates),
            ];
            let raw = self.run_git(
                &repo.root,
                &[
                    "diff-tree",
                    "-r",
                    "-z",
                    "--no-renames",
                    "--numstat",
                    base,
                    &now,
                ],
                &env,
                READ_LIMIT,
            )?;
            let status = self.run_git(
                &repo.root,
                &[
                    "diff-tree",
                    "-r",
                    "-z",
                    "--no-renames",
                    "--name-status",
                    base,
                    &now,
                ],
                &env,
                READ_LIMIT,
            )?;
            let kinds = parse_name_status(&status);
            for (rel, added, removed) in parse_numstat(&raw) {
                let kind = kinds.iter().find(|(p, _)| *p == rel).map(|(_, k)| *k);
                files.push(Changed {
                    path: repo.root.join(&rel).display().to_string(),
                    added,
                    removed,
                    created: kind == Some('A'),
                    deleted: kind == Some('D'),
                    none: None,
                });
            }
        }
        for (path, copy) in store::copies(&self.dir, &holder) {
            let shown = path.display().to_string();
            let entry = match copy {
                Copy::Before(before) => {
                    let (added, removed) = self.count(&before, &path);
                    if added == Some(0) && removed == Some(0) && path.exists() {
                        files.retain(|f| f.path != shown);
                        continue;
                    }
                    Changed {
                        path: shown.clone(),
                        added,
                        removed,
                        created: false,
                        deleted: !path.exists(),
                        none: None,
                    }
                }
                Copy::New => {
                    if !path.exists() {
                        files.retain(|f| f.path != shown);
                        continue;
                    }
                    let (added, _) = self.count(Path::new("/dev/null"), &path);
                    Changed {
                        path: shown.clone(),
                        added,
                        removed: Some(0),
                        created: true,
                        deleted: false,
                        none: None,
                    }
                }
                Copy::None(why) => Changed {
                    path: shown.clone(),
                    added: None,
                    removed: None,
                    created: false,
                    deleted: false,
                    none: Some(why),
                },
            };
            files.retain(|f| f.path != shown);
            files.push(entry);
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Changes {
            files,
            tree: tree.is_some() && repo.is_some(),
            base: meta.root.clone().unwrap_or(meta.cwd.clone()),
        })
    }

    /// Lines added and removed from `a` to `b`, through `git diff --no-index`; nulls when git
    /// cannot say (binary, or no git).
    fn count(&self, a: &Path, b: &Path) -> (Option<u64>, Option<u64>) {
        let b = if b.exists() {
            b
        } else {
            Path::new("/dev/null")
        };
        let a_s = a.display().to_string();
        let b_s = b.display().to_string();
        let mut cmd = Command::new(&self.git);
        git_env(&mut cmd);
        cmd.current_dir("/").args([
            "diff",
            "--no-index",
            "--no-renames",
            "--numstat",
            "-z",
            "--",
            &a_s,
            &b_s,
        ]);
        // Exit 1 means "they differ", which is the usual answer here.
        match process::run(&mut cmd, READ_LIMIT) {
            Ok(out) => parse_numstat(&out.stdout)
                .first()
                .map_or((Some(0), Some(0)), |(_, a, r)| (*a, *r)),
            Err(_) => (None, None),
        }
    }

    /// A file's content before the session: its first-touch copy, else the snapshot's blob.
    pub fn base_text(&self, id: &str, path: &Path) -> Result<BaseText, String> {
        let holder = self.holder(id);
        for (p, copy) in store::copies(&self.dir, &holder) {
            if p != path {
                continue;
            }
            return match copy {
                Copy::Before(c) => std::fs::read(c)
                    .map(BaseText::of)
                    .map_err(|e| e.to_string()),
                Copy::New => Ok(BaseText::missing()),
                Copy::None(why) if why == "too large" => Ok(BaseText {
                    exists: true,
                    binary: false,
                    large: true,
                    text: None,
                }),
                Copy::None(why) => Err(format!("no copy of the file: {why}")),
            };
        }
        let meta = self.meta(&holder).ok_or("no baseline for this session")?;
        let tree = self
            .ready_tree(&holder)
            .ok_or("the session has no snapshot of this file")?;
        let repo = meta
            .root
            .as_deref()
            .and_then(|r| self.repo_of(Path::new(r)))
            .ok_or("the repository is gone")?;
        let rel = path
            .strip_prefix(&repo.root)
            .map_err(|_| "the file is outside the session's repository")?;
        let dir = self.session(&holder).ok_or("not a session id")?;
        let objects = dir.join("objects");
        let alternates = repo.common.join("objects");
        let env: [(&str, &Path); 2] = [
            ("GIT_OBJECT_DIRECTORY", &objects),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", &alternates),
        ];
        let spec = format!("{tree}:{}", rel.display());
        let size = match self.run_git(&repo.root, &["cat-file", "-s", &spec], &env, READ_LIMIT) {
            Ok(out) => String::from_utf8_lossy(&out)
                .trim()
                .parse::<u64>()
                .unwrap_or(u64::MAX),
            Err(_) => return Ok(BaseText::missing()),
        };
        if size > SIZE_CAP {
            return Ok(BaseText {
                exists: true,
                binary: false,
                large: true,
                text: None,
            });
        }
        self.run_git(&repo.root, &["cat-file", "blob", &spec], &env, READ_LIMIT)
            .map(BaseText::of)
    }

    /// Retention (1.4): baselines of sessions that ended more than `keep` ago go, then the
    /// oldest ended ones until the store is under `cap`. A running session (no end) keeps
    /// its baseline, and so does one another session's baseline points at while that one runs.
    pub fn prune(&self, keep: Duration, cap: u64, now: u64) -> Vec<String> {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut sessions: Vec<(String, Option<Meta>, u64)> = read
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|id| terminescu_plugin_api::is_session_id(id))
            .map(|id| {
                let meta = self.meta(&id);
                let size = dir_size(&self.dir.join(&id));
                (id, meta, size)
            })
            .collect();
        // A session that never sent SessionEnd (Claude killed, a crash) ended when it was last
        // written: the newest of its folder, its meta file and its files folder.
        let end_of = |id: &str, m: &Option<Meta>| -> Option<u64> {
            let dir = self.dir.join(id);
            let touched = |p: PathBuf| {
                std::fs::metadata(p)
                    .and_then(|md| md.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
            };
            let last = [dir.clone(), dir.join(META), dir.join("files")]
                .into_iter()
                .filter_map(touched)
                .max();
            match m {
                Some(m) => m
                    .ended
                    .or_else(|| last.filter(|l| now.saturating_sub(*l) >= keep.as_secs())),
                // A folder without meta is a hook's copy for a session the plugin never saw
                // start.
                None => last,
            }
        };
        let ends: std::collections::HashMap<String, Option<u64>> = sessions
            .iter()
            .map(|(id, m, _)| (id.clone(), end_of(id, m)))
            .collect();
        // Sessions another running session still reads from.
        let held: std::collections::HashSet<String> = sessions
            .iter()
            .filter(|(id, m, _)| m.is_some() && ends[id].is_none())
            .filter_map(|(_, m, _)| m.as_ref().and_then(|m| m.base.clone()))
            .collect();
        let ended = |id: &str, _m: &Option<Meta>| -> Option<u64> {
            if held.contains(id) { None } else { ends[id] }
        };
        let mut gone = Vec::new();
        sessions.retain(|(id, m, _)| match ended(id, m) {
            Some(end) if now.saturating_sub(end) >= keep.as_secs() => {
                let _ = std::fs::remove_dir_all(self.dir.join(id));
                gone.push(id.clone());
                false
            }
            _ => true,
        });
        let mut total: u64 = sessions.iter().map(|(_, _, s)| s).sum();
        let mut ended_ones: Vec<(u64, String, u64)> = sessions
            .iter()
            .filter_map(|(id, m, size)| ended(id, m).map(|e| (e, id.clone(), *size)))
            .collect();
        ended_ones.sort();
        for (_, id, size) in ended_ones {
            if total <= cap {
                break;
            }
            let _ = std::fs::remove_dir_all(self.dir.join(&id));
            total = total.saturating_sub(size);
            gone.push(id);
        }
        gone
    }
}

/// A repository, as `git rev-parse` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common: PathBuf,
}

/// Whether `root` is the user's home folder, where a snapshot would walk everything.
fn is_home(root: &Path, home: Option<&Path>) -> bool {
    let Some(home) = home else { return false };
    root == home || matches!((root.canonicalize(), home.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

fn private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(read) = std::fs::read_dir(dir) else {
        return 0;
    };
    read.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// `--numstat -z` records: `added\tremoved\tpath\0`, `-` for binary.
fn parse_numstat(raw: &[u8]) -> Vec<(String, Option<u64>, Option<u64>)> {
    let mut out = Vec::new();
    let mut it = raw.split(|b| *b == 0).filter(|r| !r.is_empty());
    while let Some(rec) = it.next() {
        let text = String::from_utf8_lossy(rec);
        let mut parts = text.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        // A rename record leaves the path empty and gives the two paths next; renames are off.
        let path = if path.is_empty() {
            let _ = it.next();
            match it.next() {
                Some(p) => String::from_utf8_lossy(p).into_owned(),
                None => continue,
            }
        } else {
            path.to_string()
        };
        out.push((path, a.parse().ok(), r.parse().ok()));
    }
    out
}

/// `--name-status -z` records: `X\0path\0`.
fn parse_name_status(raw: &[u8]) -> Vec<(String, char)> {
    let mut out = Vec::new();
    let mut it = raw.split(|b| *b == 0).filter(|r| !r.is_empty());
    while let (Some(kind), Some(path)) = (it.next(), it.next()) {
        if let Some(k) = kind.first() {
            out.push((String::from_utf8_lossy(path).into_owned(), *k as char));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_is_recognised() {
        assert!(is_home(
            Path::new("/Users/owner"),
            Some(Path::new("/Users/owner"))
        ));
        assert!(!is_home(
            Path::new("/Users/owner/code"),
            Some(Path::new("/Users/owner"))
        ));
        assert!(!is_home(Path::new("/Users/owner"), None));
    }

    #[test]
    fn update_meta_keeps_what_it_does_not_change() {
        let dir = std::env::temp_dir().join(format!("tsc-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        store.set_meta("s-1", &Meta::default()).unwrap();
        // SessionEnd lands while the snapshot thread holds an older copy.
        let mut ended = store.meta("s-1").unwrap();
        ended.ended = Some(7);
        store.set_meta("s-1", &ended).unwrap();
        store
            .update_meta("s-1", |m| {
                m.snapshot_failed = Some("git took too long".into())
            })
            .unwrap();
        let got = store.meta("s-1").unwrap();
        assert_eq!(got.ended, Some(7));
        assert_eq!(got.snapshot_failed.as_deref(), Some("git took too long"));
    }

    #[test]
    fn numstat_and_name_status_parse() {
        let raw = b"3\t1\tsrc/retry.ts\0-\t-\tlogo.png\0";
        assert_eq!(
            parse_numstat(raw),
            vec![
                ("src/retry.ts".into(), Some(3), Some(1)),
                ("logo.png".into(), None, None)
            ]
        );
        assert_eq!(
            parse_name_status(b"M\0src/a.ts\0A\0b.ts\0"),
            vec![("src/a.ts".into(), 'M'), ("b.ts".into(), 'A')]
        );
    }
}
