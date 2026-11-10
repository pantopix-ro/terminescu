//! Layers around a provider (Decorator, add-git-plugin D2). The plugin builds
//! `CoalescingVcs(CachedVcs(TimedVcs(GitCli)))`: requests for one folder fold together, answers
//! are reused until the repository moves, and no read waits longer than its limit.

use crate::vcs::{Change, DiffBase, RepoFacts, Vcs, VcsError, Worktree};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Gives up on a read after `limit`; the call ends on its own thread. Changes to the
/// repository (worktree add and remove) are never cut short.
pub struct TimedVcs<V> {
    inner: Arc<V>,
    limit: Duration,
}

impl<V: Vcs + 'static> TimedVcs<V> {
    pub fn new(inner: V, limit: Duration) -> Self {
        Self {
            inner: Arc::new(inner),
            limit,
        }
    }

    fn timed<T: Send + 'static>(
        &self,
        f: impl FnOnce(&V) -> Result<T, VcsError> + Send + 'static,
    ) -> Result<T, VcsError> {
        let (tx, rx) = mpsc::channel();
        let inner = self.inner.clone();
        std::thread::Builder::new()
            .name("git-read".into())
            .spawn(move || {
                let _ = tx.send(f(&inner));
            })
            .map_err(|e| VcsError::Failed(e.to_string()))?;
        rx.recv_timeout(self.limit)
            .unwrap_or(Err(VcsError::Timeout))
    }
}

impl<V: Vcs + 'static> Vcs for TimedVcs<V> {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError> {
        let dir = dir.to_path_buf();
        self.timed(move |v| v.facts(&dir))
    }
    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError> {
        let root = root.to_path_buf();
        self.timed(move |v| v.changes(&root))
    }
    fn base_text(
        &self,
        root: &Path,
        file: &Path,
        base: DiffBase,
    ) -> Result<Option<Vec<u8>>, VcsError> {
        let (root, file) = (root.to_path_buf(), file.to_path_buf());
        self.timed(move |v| v.base_text(&root, &file, base))
    }
    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError> {
        self.inner.worktree_add(root, branch, base, at)
    }
    fn worktree_checkout(&self, root: &Path, branch: &str, at: &Path) -> Result<(), VcsError> {
        self.inner.worktree_checkout(root, branch, at)
    }
    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError> {
        let root = root.to_path_buf();
        self.timed(move |v| v.worktrees(&root))
    }
    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError> {
        self.inner.worktree_remove(root, at, force)
    }
    fn forget(&self, dir: &Path) {
        self.inner.forget(dir);
    }
}

/// Keeps the facts per folder until the repository's `HEAD` or index changes, `ttl` passes, or
/// [`Vcs::forget`] drops them. Errors are never kept. An edit to a tracked file or a new
/// untracked file touches neither, so `ttl` bounds how long the change count can lag; the
/// plugin also forgets a folder when a command ends in it.
pub struct CachedVcs<V> {
    inner: V,
    ttl: Duration,
    facts: Mutex<HashMap<PathBuf, Kept>>,
    /// Moves on each [`Vcs::forget`]: a read that started before it keeps nothing, since it
    /// may predate the command that ended.
    generation: AtomicU64,
}

struct Kept {
    facts: Option<RepoFacts>,
    at: Instant,
    stamps: Vec<Option<SystemTime>>,
}

/// The files whose change means the facts moved: this working tree's `HEAD` and index.
fn stamps(facts: &Option<RepoFacts>) -> Vec<Option<SystemTime>> {
    let Some(f) = facts else {
        return Vec::new();
    };
    ["HEAD", "index"]
        .iter()
        .map(|n| {
            std::fs::metadata(f.git_dir.join(n))
                .and_then(|m| m.modified())
                .ok()
        })
        .collect()
}

impl<V: Vcs> CachedVcs<V> {
    pub fn new(inner: V, ttl: Duration) -> Self {
        Self {
            inner,
            ttl,
            facts: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(0),
        }
    }
}

impl<V: Vcs> Vcs for CachedVcs<V> {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError> {
        if let Some(k) = lock(&self.facts).get(dir)
            && k.at.elapsed() < self.ttl
            && stamps(&k.facts) == k.stamps
        {
            return Ok(k.facts.clone());
        }
        let started = SystemTime::now();
        let generation = self.generation.load(Ordering::SeqCst);
        let facts = self.inner.facts(dir)?;
        let stamps = stamps(&facts);
        // A HEAD or index written while git ran may postdate what it read: keep nothing.
        if stamps.iter().flatten().all(|t| *t < started) {
            let kept = Kept {
                stamps,
                facts: facts.clone(),
                at: Instant::now(),
            };
            let mut all = lock(&self.facts);
            // Checked under the lock `forget` takes, so a forget never slips in between.
            if self.generation.load(Ordering::SeqCst) == generation {
                all.insert(dir.to_path_buf(), kept);
            }
        }
        Ok(facts)
    }
    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError> {
        self.inner.changes(root)
    }
    fn base_text(
        &self,
        root: &Path,
        file: &Path,
        base: DiffBase,
    ) -> Result<Option<Vec<u8>>, VcsError> {
        self.inner.base_text(root, file, base)
    }
    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError> {
        self.inner.worktree_add(root, branch, base, at)
    }
    fn worktree_checkout(&self, root: &Path, branch: &str, at: &Path) -> Result<(), VcsError> {
        self.inner.worktree_checkout(root, branch, at)
    }
    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError> {
        self.inner.worktrees(root)
    }
    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError> {
        self.inner.worktree_remove(root, at, force)
    }
    /// Drops `dir` and every folder of a working tree that holds it, asked about or not.
    fn forget(&self, dir: &Path) {
        let mut kept = lock(&self.facts);
        self.generation.fetch_add(1, Ordering::SeqCst);
        let roots: Vec<PathBuf> = kept
            .iter()
            .filter_map(|(d, k)| Some((d, k.facts.as_ref()?)))
            .filter(|(d, f)| d.as_path() == dir || dir.starts_with(&f.root))
            .map(|(_, f)| f.root.clone())
            .collect();
        kept.retain(|d, k| d != dir && k.facts.as_ref().is_none_or(|f| !roots.contains(&f.root)));
        drop(kept);
        self.inner.forget(dir);
    }
}

/// Folds requests for one folder that arrive while a call for it runs into one follow-up call,
/// whose answer they all share. Every answer comes from a call that started after the request.
pub struct CoalescingVcs<V> {
    inner: V,
    facts: Fold<Result<Option<RepoFacts>, VcsError>>,
    changes: Fold<Result<Vec<Change>, VcsError>>,
}

impl<V: Vcs> CoalescingVcs<V> {
    pub fn new(inner: V) -> Self {
        Self {
            inner,
            facts: Fold::default(),
            changes: Fold::default(),
        }
    }
}

impl<V: Vcs> Vcs for CoalescingVcs<V> {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError> {
        self.facts.run(dir, || self.inner.facts(dir))
    }
    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError> {
        self.changes.run(root, || self.inner.changes(root))
    }
    fn base_text(
        &self,
        root: &Path,
        file: &Path,
        base: DiffBase,
    ) -> Result<Option<Vec<u8>>, VcsError> {
        self.inner.base_text(root, file, base)
    }
    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError> {
        self.inner.worktree_add(root, branch, base, at)
    }
    fn worktree_checkout(&self, root: &Path, branch: &str, at: &Path) -> Result<(), VcsError> {
        self.inner.worktree_checkout(root, branch, at)
    }
    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError> {
        self.inner.worktrees(root)
    }
    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError> {
        self.inner.worktree_remove(root, at, force)
    }
    fn forget(&self, dir: &Path) {
        self.inner.forget(dir);
    }
}

/// One call at a time per key; the requests that wait share the next call's answer.
struct Fold<T> {
    slots: Mutex<HashMap<PathBuf, Slot<T>>>,
    ended: Condvar,
}

impl<T> Default for Fold<T> {
    fn default() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            ended: Condvar::new(),
        }
    }
}

struct Slot<T> {
    running: bool,
    /// Calls started and finished, numbered in order.
    started: u64,
    finished: u64,
    last: Option<T>,
    waiting: usize,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            running: false,
            started: 0,
            finished: 0,
            last: None,
            waiting: 0,
        }
    }
}

impl<T: Clone> Fold<T> {
    fn run(&self, key: &Path, call: impl FnOnce() -> T) -> T {
        let mut call = Some(call);
        let mut slots = lock(&self.slots);
        let want = {
            let s = slots.entry(key.to_path_buf()).or_default();
            s.waiting += 1;
            s.started + 1
        };
        loop {
            let s = slots.get_mut(key).expect("held while waiting");
            if s.finished >= want {
                let answer = s.last.clone().expect("a finished call left its answer");
                self.leave(&mut slots, key);
                return answer;
            }
            if !s.running {
                s.running = true;
                s.started += 1;
                let id = s.started;
                drop(slots);
                let run = catch_unwind(AssertUnwindSafe(call.take().expect("called once")));
                slots = lock(&self.slots);
                let s = slots.get_mut(key).expect("held while running");
                s.running = false;
                if let Ok(answer) = &run {
                    s.finished = id;
                    s.last = Some(answer.clone());
                }
                self.leave(&mut slots, key);
                self.ended.notify_all();
                return run.unwrap_or_else(|p| resume_unwind(p));
            }
            slots = self.ended.wait(slots).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// One request is done; the last one out takes the slot away.
    fn leave(&self, slots: &mut HashMap<PathBuf, Slot<T>>, key: &Path) {
        if let Some(s) = slots.get_mut(key) {
            s.waiting -= 1;
            if s.waiting == 0 && !s.running {
                slots.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcs::Head;
    use std::sync::atomic::AtomicUsize;

    /// A provider that counts its calls, can be slow, and can be held at a gate.
    #[derive(Default)]
    struct Fake {
        calls: AtomicUsize,
        delay: Duration,
        git_dir: PathBuf,
        gate: Arc<(Mutex<bool>, Condvar)>,
        gated: bool,
        /// Writes HEAD during each call, as a commit in another shell would.
        touch: bool,
    }

    impl Fake {
        fn facts_now(&self) -> RepoFacts {
            RepoFacts {
                root: PathBuf::from("/code/shop-api"),
                git_dir: self.git_dir.clone(),
                common_dir: self.git_dir.clone(),
                head: Head::Branch("main".into()),
                upstream: None,
                ahead: 0,
                behind: 0,
                changed: self.calls.load(Ordering::SeqCst) as u32,
                conflicted: 0,
                op: None,
            }
        }
    }

    impl Vcs for Fake {
        fn facts(&self, _: &Path) -> Result<Option<RepoFacts>, VcsError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.gated {
                let (open, cv) = &*self.gate;
                let mut o = lock(open);
                while !*o {
                    o = cv.wait(o).unwrap();
                }
            }
            std::thread::sleep(self.delay);
            if self.touch {
                std::fs::write(self.git_dir.join("HEAD"), "ref: refs/heads/next\n").unwrap();
            }
            Ok(Some(self.facts_now()))
        }
        fn changes(&self, _: &Path) -> Result<Vec<Change>, VcsError> {
            Ok(Vec::new())
        }
        fn base_text(&self, _: &Path, _: &Path, _: DiffBase) -> Result<Option<Vec<u8>>, VcsError> {
            Ok(None)
        }
        fn worktree_add(&self, _: &Path, _: &str, _: &str, _: &Path) -> Result<(), VcsError> {
            std::thread::sleep(self.delay);
            Ok(())
        }
        fn worktree_checkout(&self, _: &Path, _: &str, _: &Path) -> Result<(), VcsError> {
            Ok(())
        }
        fn worktrees(&self, _: &Path) -> Result<Vec<Worktree>, VcsError> {
            Ok(Vec::new())
        }
        fn worktree_remove(&self, _: &Path, _: &Path, _: bool) -> Result<(), VcsError> {
            Ok(())
        }
    }

    fn git_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "terminescu-layers-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(dir.join("index"), "i").unwrap();
        dir
    }

    #[test]
    fn a_slow_read_is_dropped_at_the_limit() {
        let slow = Fake {
            delay: Duration::from_millis(400),
            ..Fake::default()
        };
        let timed = TimedVcs::new(slow, Duration::from_millis(50));
        let start = Instant::now();
        assert_eq!(timed.facts(Path::new("/code")), Err(VcsError::Timeout));
        assert!(start.elapsed() < Duration::from_millis(300));
        // A change to the repository is never cut short.
        assert_eq!(
            timed.worktree_add(Path::new("/code"), "b", "main", Path::new("/w")),
            Ok(())
        );
        let fast = TimedVcs::new(Fake::default(), Duration::from_millis(500));
        assert!(fast.facts(Path::new("/code")).unwrap().is_some());
    }

    #[test]
    fn kept_facts_last_until_head_or_the_index_moves() {
        let dir = git_dir();
        let cached = CachedVcs::new(
            Fake {
                git_dir: dir.clone(),
                ..Fake::default()
            },
            Duration::from_secs(60),
        );
        let at = Path::new("/code/shop-api");
        let first = cached.facts(at).unwrap();
        assert_eq!(cached.facts(at).unwrap(), first);
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 1);
        // A new mtime on the index (a commit, an add) asks again.
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(dir.join("index"), "j").unwrap();
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 2);
        // So does a command that ended anywhere in the working tree, asked about or not.
        cached.forget(Path::new("/code/shop-api/src"));
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 3);
        cached.forget(at);
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 4);
        // A folder outside it leaves the kept facts alone.
        cached.forget(Path::new("/code/shop-web"));
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn facts_read_while_head_moves_are_not_kept() {
        let dir = git_dir();
        let cached = CachedVcs::new(
            Fake {
                git_dir: dir.clone(),
                touch: true,
                ..Fake::default()
            },
            Duration::from_secs(60),
        );
        let at = Path::new("/code/shop-api");
        cached.facts(at).unwrap();
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn kept_facts_expire() {
        let cached = CachedVcs::new(
            Fake {
                git_dir: git_dir(),
                ..Fake::default()
            },
            Duration::from_millis(30),
        );
        let at = Path::new("/code/shop-api");
        cached.facts(at).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn requests_during_a_call_fold_into_one_follow_up() {
        let fake = Fake {
            gated: true,
            ..Fake::default()
        };
        let gate = fake.gate.clone();
        let vcs = Arc::new(CoalescingVcs::new(fake));
        let at = Path::new("/code/shop-api");
        let first = {
            let vcs = vcs.clone();
            std::thread::spawn(move || vcs.facts(at).unwrap().unwrap().changed)
        };
        while vcs.inner.calls.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
        let later: Vec<_> = (0..5)
            .map(|_| {
                let vcs = vcs.clone();
                std::thread::spawn(move || vcs.facts(at).unwrap().unwrap().changed)
            })
            .collect();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(vcs.inner.calls.load(Ordering::SeqCst), 1);
        *lock(&gate.0) = true;
        gate.1.notify_all();
        // The first call answers its own request; the five share the one call after it.
        assert_eq!(first.join().unwrap(), 1);
        for t in later {
            assert_eq!(t.join().unwrap(), 2);
        }
        assert_eq!(vcs.inner.calls.load(Ordering::SeqCst), 2);
        assert!(lock(&vcs.facts.slots).is_empty());
    }

    #[test]
    fn a_read_that_started_before_a_forget_is_not_kept() {
        let fake = Fake {
            git_dir: git_dir(),
            gated: true,
            ..Fake::default()
        };
        let gate = fake.gate.clone();
        let cached = Arc::new(CachedVcs::new(fake, Duration::from_secs(60)));
        let at = Path::new("/code/shop-api");
        let reading = {
            let cached = cached.clone();
            std::thread::spawn(move || cached.facts(at).unwrap())
        };
        while cached.inner.calls.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
        // A command ends while git still reads.
        cached.forget(at);
        *lock(&gate.0) = true;
        gate.1.notify_all();
        reading.join().unwrap();
        assert!(lock(&cached.facts).is_empty());
        cached.facts(at).unwrap();
        assert_eq!(cached.inner.calls.load(Ordering::SeqCst), 2);
    }
}
