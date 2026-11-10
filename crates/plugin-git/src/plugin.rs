//! The git plugin (add-git-plugin D3, D5, D7). Shell events mark when a pane's repository may
//! have moved; each sets a 300 ms timer for the pane, and the timer reads the facts. The tab
//! shows the facts of its lead pane, the one that last moved or ran a command. With `gh` on,
//! the lead pane's branch also looks up its pull request, on a thread of its own, at most once
//! a minute and after each command; short timers pick the answer up (D4).

use crate::forge::{Checks, Forge, ForgeError, GhCli, PullRequest};
use crate::vcs::{DiffBase, RepoFacts, Vcs, VcsError};
use crate::worktree::{path_for, slug};
use crate::{CachedVcs, CoalescingVcs, GitCli, TimedVcs};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use terminescu_plugin_api::{
    CoreEvent, Decoration, Id, Manifest, PaneInfo, Plugin, PluginCx, PluginError, Slot, Target,
    Tone,
};

/// How long a pane waits after its last event before git runs.
pub const DEBOUNCE: Duration = Duration::from_millis(300);
/// A base text larger than this stays in git: the diff tab says so instead.
pub const MAX_BASE: usize = 4 << 20;
/// The Changes section lists at most this many files, and the total.
pub const MAX_CHANGES: usize = 500;
/// Settings key: the folder worktrees go under; absent puts them next to the repository.
pub const WORKTREE_FOLDER: &str = "worktreeFolder";
/// Row data key: the worktree a row was made for, and its repository.
pub const WORKTREE_ROW: &str = "worktree";
/// Settings key: `false` turns the GitHub part off.
pub const GITHUB: &str = "github";
/// A branch's pull request is looked up at most this often, unless a command ended.
pub const PR_EVERY: Duration = Duration::from_secs(60);
/// How often a pane looks for the answer of a lookup that runs.
pub const PR_POLL: Duration = Duration::from_millis(250);
/// An answer younger than this stands; the margin keeps the minute tick from just missing it.
const PR_FRESH: Duration = Duration::from_secs(58);

/// After a `git --version` that timed out at launch, as the first run after a reboot can.
pub const VERSION_RETRY: Duration = Duration::from_secs(5);

const TIMER: &str = "pane:";
/// Asks `git --version` again.
pub const VERSION_TIMER: &str = "git:version";
const PR_READY: &str = "prready:";
const PR_TICK: &str = "prtick:";

/// A branch of a working tree.
type Branch = (PathBuf, String);

#[derive(Default)]
struct Prs {
    known: HashMap<Branch, Known>,
    running: HashSet<Branch>,
    /// Branches whose command ended while their lookup ran: that answer may be stale.
    again: HashSet<Branch>,
    /// Set by a lookup that found `gh` gone or logged out.
    lost: Option<String>,
}

struct Known {
    /// When the lookup started, so the minute tick finds the answer due.
    at: Instant,
    pr: Option<PullRequest>,
}

/// Takes a branch out of the running lookups when dropped armed, as on a panic.
struct Running(Arc<Mutex<Prs>>, Option<Branch>);

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(key) = self.1.take() {
            lock(&self.0).running.remove(&key);
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct GitPlugin {
    vcs: Arc<dyn Vcs>,
    /// Checked on launch; `None` in tests that bring their own provider.
    git: Option<GitCli>,
    forge: Option<Arc<dyn Forge>>,
    prs: Arc<Mutex<Prs>>,
    state: Mutex<State>,
    /// What each tab shows now, so a publish changes only what differs (polish-git-plugin D2).
    shown: Mutex<HashMap<Id, Shown>>,
}

/// A tab's chips and facts as last published.
#[derive(Default, PartialEq)]
struct Shown {
    branch: Option<Decoration>,
    pr: Option<Decoration>,
    facts: [Option<Value>; 3],
}

/// The tab facts for status rules, in [`Shown::facts`] order.
const TAB_FACTS: [&str; 3] = ["pushed", "conflict", "checksFailed"];

#[derive(Default)]
struct State {
    panes: HashMap<Id, PaneGit>,
    /// Per tab, the pane whose facts the tab shows.
    lead: HashMap<Id, Id>,
    /// Per row, its window: kept until the row closes, after its panes went (D6).
    windows: HashMap<Id, Id>,
    /// Why the plugin stopped: no git, or one too old.
    stopped: Option<String>,
    version: Option<String>,
    /// The GitHub part answers: `gh` is there and logged in, and the setting allows it.
    github: bool,
    /// Why the GitHub part is off.
    github_off: Option<String>,
}

struct PaneGit {
    tab: Id,
    side_tab: Id,
    window: Id,
    cwd: Option<String>,
    facts: Option<RepoFacts>,
    /// Facts were read for the current cwd; until then `None` means unknown, not "no repo".
    read: bool,
    /// A command ended: the next read looks up the pull request whatever its age.
    pr_due: bool,
}

impl PaneGit {
    fn from(p: &PaneInfo) -> Self {
        Self {
            tab: p.tab.clone(),
            side_tab: p.side_tab.clone(),
            window: p.window.clone(),
            cwd: p.cwd.clone(),
            facts: None,
            read: false,
            pr_due: false,
        }
    }
}

impl Default for GitPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl GitPlugin {
    /// The person's `git`, read through the layers D2 names.
    pub fn new() -> Self {
        let git = GitCli::default();
        let vcs = CoalescingVcs::new(CachedVcs::new(
            TimedVcs::new(git.clone(), Duration::from_secs(1)),
            Duration::from_secs(2),
        ));
        Self {
            vcs: Arc::new(vcs),
            git: Some(git),
            forge: Some(Arc::new(GhCli::default())),
            prs: Arc::default(),
            state: Mutex::default(),
            shown: Mutex::default(),
        }
    }

    /// Reads through `vcs`, skips the version check and has no forge.
    pub fn with_vcs(vcs: Arc<dyn Vcs>) -> Self {
        Self {
            vcs,
            git: None,
            forge: None,
            prs: Arc::default(),
            state: Mutex::default(),
            shown: Mutex::default(),
        }
    }

    /// Checks this `git`'s version on launch, as [`GitPlugin::new`] does with the person's.
    pub fn with_git(mut self, git: GitCli) -> Self {
        self.git = Some(git);
        self
    }

    pub fn with_forge(mut self, forge: Arc<dyn Forge>) -> Self {
        self.forge = Some(forge);
        self
    }

    /// Whether the GitHub part can answer now, and why not.
    fn check_github(&self, cx: &dyn PluginCx) -> Result<(), String> {
        if cx.setting(GITHUB) == Some(Value::Bool(false)) {
            return Err("GitHub is off in Settings".into());
        }
        let forge = self.forge.as_ref().ok_or("no forge")?;
        forge.available().map_err(|e| e.to_string())
    }

    /// Tells the UI half whether the GitHub items show (`github` event, `{ on }`).
    fn set_github(&self, cx: &dyn PluginCx) {
        let check = self.check_github(cx);
        lock(&self.prs).lost = None;
        let on = check.is_ok();
        let mut st = lock(&self.state);
        st.github = on;
        st.github_off = check.err();
        drop(st);
        cx.emit("github", json!({ "on": on }));
    }

    /// Checks `git --version`; false when the plugin stopped for it. A slow answer is no
    /// verdict: the plugin keeps going and asks again after [`VERSION_RETRY`].
    fn check_version(&self, cx: &dyn PluginCx) -> bool {
        let Some(git) = &self.git else {
            return true;
        };
        match git.version() {
            Ok(v) => {
                lock(&self.state).version = Some(v);
                true
            }
            Err(VcsError::Timeout) => {
                eprintln!("terminescu: git --version timed out; asking again");
                cx.schedule(VERSION_RETRY, VERSION_TIMER);
                true
            }
            Err(e) => {
                let why = e.to_string();
                eprintln!("terminescu: git plugin stopped: {why}");
                lock(&self.state).stopped = Some(why);
                cx.clear_decorations(None);
                lock(&self.shown).clear();
                false
            }
        }
    }

    fn launch(&self, cx: &dyn PluginCx) {
        cx.clear_decorations(None);
        lock(&self.shown).clear();
        let mut st = lock(&self.state);
        *st = State::default();
        drop(st);
        if !self.check_version(cx) {
            return;
        }
        self.set_github(cx);
        let mut st = lock(&self.state);
        let panes = cx.panes();
        for p in &panes {
            st.lead
                .entry(p.tab.clone())
                .or_insert_with(|| p.pane.clone());
            st.windows.insert(p.side_tab.clone(), p.window.clone());
            st.panes.insert(p.pane.clone(), PaneGit::from(p));
        }
        drop(st);
        for p in panes {
            cx.schedule(Duration::ZERO, &format!("{TIMER}{}", p.pane));
        }
    }

    /// The pane is active: it leads its tab, and its facts refresh after the debounce.
    fn touch(&self, pane: &str, cwd: Option<&str>, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        if st.stopped.is_some() {
            return;
        }
        if !st.panes.contains_key(pane) {
            let Some(p) = cx.panes().into_iter().find(|p| p.pane == pane) else {
                return;
            };
            st.windows.insert(p.side_tab.clone(), p.window.clone());
            st.panes.insert(p.pane.clone(), PaneGit::from(&p));
        }
        let p = st.panes.get_mut(pane).expect("just checked");
        if let Some(cwd) = cwd
            && p.cwd.as_deref() != Some(cwd)
        {
            p.cwd = Some(cwd.into());
            p.read = false;
        }
        let tab = p.tab.clone();
        let led = st.lead.insert(tab.clone(), pane.into());
        let known = st.panes[pane].read.then(|| st.panes[pane].facts.clone());
        drop(st);
        // A new lead shows what it knows now; the refresh brings it up to date.
        if led.as_deref() != Some(pane)
            && let Some(facts) = known
        {
            self.show(cx, &tab, facts.as_ref());
        }
        cx.schedule(DEBOUNCE, &format!("{TIMER}{pane}"));
    }

    fn refresh(&self, pane: &str, cx: &dyn PluginCx) {
        let cwd = {
            let st = lock(&self.state);
            if st.stopped.is_some() {
                return;
            }
            match st.panes.get(pane).and_then(|p| p.cwd.clone()) {
                Some(cwd) => cwd,
                None => return,
            }
        };
        let facts = match self.vcs.facts(Path::new(&cwd)) {
            Ok(f) => f,
            // Kept as they were; the next trigger tries again.
            Err(VcsError::Timeout) => return,
            Err(e @ (VcsError::NoGit | VcsError::TooOld(_))) => {
                let why = e.to_string();
                eprintln!("terminescu: git plugin stopped: {why}");
                lock(&self.state).stopped = Some(why);
                cx.clear_decorations(None);
                lock(&self.shown).clear();
                return;
            }
            Err(e) => {
                eprintln!("terminescu: git facts for {cwd}: {e}");
                return;
            }
        };
        let mut st = lock(&self.state);
        let Some(p) = st.panes.get_mut(pane) else {
            return;
        };
        // The pane moved on while git ran; its own timer reads the new folder.
        if p.cwd.as_deref() != Some(cwd.as_str()) {
            return;
        }
        p.facts = facts.clone();
        p.read = true;
        let due = std::mem::take(&mut p.pr_due);
        let tab = p.tab.clone();
        let leads = st.lead.get(&tab).map(String::as_str) == Some(pane);
        drop(st);
        // The Changes section reads its list again; a dirty file edited again keeps the
        // same facts, so this goes out on every read. Diff tabs read again only for their
        // own root (D3).
        let root = facts.as_ref().map(|f| f.root.clone());
        cx.emit("facts", json!({ "pane": pane, "root": root }));
        let target = Target::Pane(pane.into());
        let f = facts.as_ref();
        cx.set_variable(
            &target,
            "branch",
            f.and_then(|f| f.head.branch().map(String::from)),
        );
        cx.set_variable(&target, "repo", f.map(RepoFacts::repo_name));
        let pr = self.known_pr(f);
        cx.set_variable(&target, "pr", pr.map(|p| p.number.to_string()));
        if leads {
            self.show(cx, &tab, f);
            if let Some(key) = f.and_then(branch_of) {
                self.look_up(pane, key, due, cx);
            }
        }
    }

    /// The pull request last found for the facts' branch.
    fn known_pr(&self, facts: Option<&RepoFacts>) -> Option<PullRequest> {
        if !lock(&self.state).github {
            return None;
        }
        let key = branch_of(facts?)?;
        lock(&self.prs).known.get(&key)?.pr.clone()
    }

    /// What a tab shows for its lead pane's facts, and the pull request of their branch.
    fn show(&self, cx: &dyn PluginCx, tab: &str, facts: Option<&RepoFacts>) {
        let pr = self.known_pr(facts);
        let github = lock(&self.state).github && facts.and_then(branch_of).is_some();
        self.publish(cx, tab, facts, pr.as_ref(), github);
    }

    /// Starts a lookup of the branch's pull request unless one runs or a recent answer stands,
    /// and keeps the minute tick going for the pane.
    fn look_up(&self, pane: &str, key: Branch, due: bool, cx: &dyn PluginCx) {
        let Some(forge) = self.forge.clone() else {
            return;
        };
        if !lock(&self.state).github {
            return;
        }
        cx.schedule(PR_EVERY, &format!("{PR_TICK}{pane}"));
        let mut prs = lock(&self.prs);
        if prs.running.contains(&key) {
            if due {
                prs.again.insert(key);
            }
            drop(prs);
            cx.schedule(PR_POLL, &format!("{PR_READY}{pane}"));
            return;
        }
        let fresh = prs
            .known
            .get(&key)
            .is_some_and(|k| k.at.elapsed() < PR_FRESH);
        if fresh && !due {
            return;
        }
        prs.running.insert(key.clone());
        drop(prs);
        let shared = self.prs.clone();
        let at = Instant::now();
        let started = std::thread::Builder::new()
            .name("git-pr".into())
            .spawn(move || {
                // Leaves `running` even when `gh` handling panics, or the branch is never
                // looked up again.
                let mut guard = Running(shared.clone(), Some(key.clone()));
                let found = forge.pr_for(&key.0, &key.1);
                let mut prs = lock(&shared);
                // The answer and the end of the run land under one lock, so a poll never
                // sees the run over with the old answer.
                prs.running.remove(&key);
                guard.1 = None;
                let pr = match found {
                    Ok(pr) => pr,
                    Err(e @ (ForgeError::NoGh | ForgeError::LoggedOut)) => {
                        prs.lost = Some(e.to_string());
                        None
                    }
                    // Slow or failed: the last answer stands until the next minute.
                    Err(e) => {
                        eprintln!("terminescu: pull request for {}: {e}", key.1);
                        prs.known.get(&key).and_then(|k| k.pr.clone())
                    }
                };
                prs.known.insert(key, Known { at, pr });
            });
        match started {
            Ok(_) => cx.schedule(PR_POLL, &format!("{PR_READY}{pane}")),
            Err(e) => {
                eprintln!("terminescu: pull request lookup not started: {e}");
                lock(&self.prs).running.clear();
            }
        }
    }

    /// A lookup may have answered: show it on the pane's tab, or look again shortly.
    fn pr_ready(&self, pane: &str, cx: &dyn PluginCx) {
        if let Some(why) = lock(&self.prs).lost.take() {
            eprintln!("terminescu: GitHub part off: {why}");
            let mut st = lock(&self.state);
            st.github = false;
            st.github_off = Some(why);
            drop(st);
            cx.emit("github", json!({ "on": false }));
        }
        let st = lock(&self.state);
        let Some(p) = st.panes.get(pane) else {
            return;
        };
        if st.lead.get(&p.tab).map(String::as_str) != Some(pane) {
            return;
        }
        let (tab, facts) = (p.tab.clone(), p.facts.clone());
        drop(st);
        let running = facts
            .as_ref()
            .and_then(branch_of)
            .is_some_and(|k| lock(&self.prs).running.contains(&k));
        if running {
            cx.schedule(PR_POLL, &format!("{PR_READY}{pane}"));
            return;
        }
        let again = facts
            .as_ref()
            .and_then(branch_of)
            .filter(|k| lock(&self.prs).again.remove(k));
        let pr = self.known_pr(facts.as_ref());
        cx.set_variable(
            &Target::Pane(pane.into()),
            "pr",
            pr.map(|p| p.number.to_string()),
        );
        self.show(cx, &tab, facts.as_ref());
        if let Some(key) = again {
            self.look_up(pane, key, true, cx);
        }
    }

    /// The minute tick looks up the pull request of the facts the pane has, reading no git
    /// (D1). A pane with no branch lets its tick go; a refresh that finds one starts it again.
    fn tick(&self, pane: &str, cx: &dyn PluginCx) {
        let key = {
            let st = lock(&self.state);
            let Some(p) = st.panes.get(pane) else {
                return;
            };
            if !st.github || st.lead.get(&p.tab).map(String::as_str) != Some(pane) {
                return;
            }
            match p.facts.as_ref().and_then(branch_of) {
                Some(key) => key,
                None => return,
            }
        };
        self.look_up(pane, key, false, cx);
    }

    fn pane_closed(&self, pane: &str, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        let Some(gone) = st.panes.remove(pane) else {
            return;
        };
        if st.lead.get(&gone.tab).map(String::as_str) != Some(pane) {
            return;
        }
        let next = st
            .panes
            .iter()
            .find(|(_, p)| p.tab == gone.tab)
            .map(|(id, p)| (id.clone(), p.facts.clone()));
        match &next {
            Some((id, _)) => st.lead.insert(gone.tab.clone(), id.clone()),
            None => st.lead.remove(&gone.tab),
        };
        drop(st);
        self.show(cx, &gone.tab, next.as_ref().and_then(|n| n.1.as_ref()));
        if let Some((id, _)) = next {
            cx.schedule(Duration::ZERO, &format!("{TIMER}{id}"));
        }
    }

    fn tab_closed(&self, tab: &str, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        st.lead.remove(tab);
        st.panes.retain(|_, p| p.tab != tab);
        drop(st);
        self.show(cx, tab, None);
    }

    /// The row's tabs show nothing more. A worktree mark still in the row's data means no one
    /// asked about the worktree, as when the row's shell exited: the UI half offers to remove
    /// it (D6).
    fn side_tab_closed(&self, side_tab: &str, data: &BTreeMap<String, Value>, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        let window = st.windows.remove(side_tab);
        let tabs: Vec<Id> = st
            .panes
            .values()
            .filter(|p| p.side_tab == side_tab)
            .map(|p| p.tab.clone())
            .collect();
        st.panes.retain(|_, p| p.side_tab != side_tab);
        for t in &tabs {
            st.lead.remove(t);
        }
        drop(st);
        for t in tabs {
            self.show(cx, &t, None);
        }
        if let Some(mark) = data.get(WORKTREE_ROW)
            && let Some(worktree) = mark.get("worktree").and_then(Value::as_str)
        {
            cx.emit(
                "orphan",
                json!({
                    "row": side_tab,
                    "window": window,
                    "worktree": worktree,
                    "repo": mark.get("repo"),
                    "branch": mark.get("branch"),
                }),
            );
        }
    }

    fn selected(&self, side_tab: &str, cx: &dyn PluginCx) {
        let panes: Vec<Id> = lock(&self.state)
            .panes
            .iter()
            .filter(|(_, p)| p.side_tab == side_tab)
            .map(|(id, _)| id.clone())
            .collect();
        for p in panes {
            cx.schedule(DEBOUNCE, &format!("{TIMER}{p}"));
        }
    }

    /// The pane's place and the facts of its folder, read now.
    fn pane_facts(
        &self,
        args: &Value,
        cx: &dyn PluginCx,
    ) -> Result<(Located, RepoFacts), PluginError> {
        let pane = arg(args, "pane")?;
        let located = {
            let st = lock(&self.state);
            if let Some(why) = &st.stopped {
                return Err(PluginError::Failed(why.clone()));
            }
            st.panes.get(pane).map(|p| Located {
                side_tab: p.side_tab.clone(),
                window: p.window.clone(),
                cwd: p.cwd.clone(),
            })
        };
        let located = match located {
            Some(l) => l,
            None => cx
                .panes()
                .into_iter()
                .find(|p| p.pane == pane)
                .map(|p| Located {
                    side_tab: p.side_tab,
                    window: p.window,
                    cwd: p.cwd,
                })
                .ok_or_else(|| PluginError::BadArgs(format!("no pane {pane}")))?,
        };
        let cwd = located
            .cwd
            .clone()
            .ok_or_else(|| PluginError::Failed("the pane has no folder yet".into()))?;
        let facts = self
            .vcs
            .facts(Path::new(&cwd))
            .map_err(failed)?
            .ok_or_else(|| PluginError::Failed("the pane is not in a git repository".into()))?;
        Ok((located, facts))
    }

    fn changes(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (_, facts) = self.pane_facts(args, cx)?;
        let mut changes = self.vcs.changes(&facts.root).map_err(failed)?;
        let total = changes.len();
        changes.truncate(MAX_CHANGES);
        Ok(json!({ "root": facts.root, "total": total, "changes": changes }))
    }

    /// The facts of `root`, a working tree's folder, as a diff tab names it; else of `pane`.
    fn root_facts(&self, args: &Value, cx: &dyn PluginCx) -> Result<RepoFacts, PluginError> {
        let Some(root) = args.get("root").and_then(Value::as_str) else {
            return self.pane_facts(args, cx).map(|(_, f)| f);
        };
        if let Some(why) = &lock(&self.state).stopped {
            return Err(PluginError::Failed(why.clone()));
        }
        let root = Path::new(root);
        if !root.is_absolute() {
            return Err(PluginError::BadArgs("root is an absolute folder".into()));
        }
        self.vcs
            .facts(root)
            .map_err(failed)?
            .ok_or_else(|| PluginError::Failed("the folder is not in a git repository".into()))
    }

    fn base_text(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let facts = self.root_facts(args, cx)?;
        // A renamed file compares with its old path at the base (D7).
        let path = arg(args, "from").or_else(|_| arg(args, "path"))?;
        let base: DiffBase = match args.get("base") {
            None | Some(Value::Null) => DiffBase::Head,
            Some(b) => serde_json::from_value(b.clone())
                .map_err(|_| PluginError::BadArgs("base is head or index".into()))?,
        };
        let text = self
            .vcs
            .base_text(&facts.root, Path::new(path), base)
            .map_err(failed)?;
        Ok(match text {
            None => json!({ "root": facts.root, "exists": false }),
            Some(bytes) if bytes.len() > MAX_BASE => {
                json!({ "root": facts.root, "exists": true, "binary": false, "size": bytes.len(), "large": true })
            }
            Some(bytes) => {
                let size = bytes.len();
                match String::from_utf8(bytes) {
                    Ok(t) if !t.contains('\0') => {
                        json!({ "root": facts.root, "exists": true, "binary": false, "size": size, "text": t })
                    }
                    _ => {
                        json!({ "root": facts.root, "exists": true, "binary": true, "size": size })
                    }
                }
            }
        })
    }

    /// Where New Row from Worktree puts the worktree, with `<branch>` for the branch, and the
    /// base it starts from when the person names none (D8).
    fn worktree_where(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (_, facts) = self.pane_facts(args, cx)?;
        let (repo, main) = (slug(&facts.repo_name()), facts.main_root());
        let at = match worktree_folder(cx) {
            Some(f) => f.join(repo).join("<branch>"),
            None => main
                .parent()
                .unwrap_or(&main)
                .join(format!("{repo}-<branch>")),
        };
        let base = facts.head.branch().unwrap_or("HEAD");
        Ok(json!({ "at": at, "base": base }))
    }

    fn worktree_add(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (at_pane, facts) = self.pane_facts(args, cx)?;
        let branch = arg(args, "branch")?.trim();
        let base = match args.get("base").and_then(Value::as_str) {
            Some(b) if !b.trim().is_empty() => b.trim().to_string(),
            _ => facts.head.branch().unwrap_or("HEAD").to_string(),
        };
        let folder = worktree_folder(cx);
        let main = facts.main_root();
        let at = path_for(&main, &facts.repo_name(), branch, folder.as_deref());
        self.vcs
            .worktree_add(&facts.root, branch, &base, &at)
            .map_err(failed)?;
        self.vcs.forget(&main);
        let mark = json!({ "worktree": at, "repo": main, "branch": branch });
        let row = self.marked_row(&at_pane, branch, &at, &main, mark, cx)?;
        Ok(json!({ "row": row, "path": at, "branch": branch, "base": base }))
    }

    /// A row in the new worktree `at`, after the pane's row, carrying `mark`. When the row
    /// cannot be made, the worktree goes again (never with `--force`), so a retry finds the
    /// path free; the branch stays.
    fn marked_row(
        &self,
        at_pane: &Located,
        title: &str,
        at: &Path,
        main: &Path,
        mark: Value,
        cx: &dyn PluginCx,
    ) -> Result<Id, PluginError> {
        let cwd = at.to_string_lossy();
        let made = cx
            .new_row(&at_pane.window, Some(&at_pane.side_tab), title, &cwd)
            .and_then(|row| {
                cx.set_row_data(&row, WORKTREE_ROW, Some(mark))?;
                Ok(row)
            });
        made.map_err(|e| {
            let undone = match self.vcs.worktree_remove(main, at, false) {
                Ok(()) => "the worktree was removed again".to_string(),
                Err(rm) => format!("the worktree stays at {}: {rm}", at.display()),
            };
            self.vcs.forget(main);
            PluginError::Failed(format!("no row for the worktree ({e}); {undone}"))
        })
    }

    fn forge(&self) -> Result<Arc<dyn Forge>, PluginError> {
        let st = lock(&self.state);
        match (&self.forge, st.github) {
            (Some(f), true) => Ok(f.clone()),
            _ => Err(PluginError::Failed(
                st.github_off
                    .clone()
                    .unwrap_or_else(|| "GitHub is off".into()),
            )),
        }
    }

    fn pull_requests(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let forge = self.forge()?;
        let (_, facts) = self.pane_facts(args, cx)?;
        let prs = forge
            .open_prs(&facts.root)
            .map_err(|e| PluginError::Failed(e.to_string()))?;
        Ok(json!(prs))
    }

    /// A worktree for pull request `number`, checked out by `gh`, opened as a marked row.
    fn pr_checkout(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let forge = self.forge()?;
        let (at_pane, facts) = self.pane_facts(args, cx)?;
        let number = args
            .get("number")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| PluginError::BadArgs("number is missing".into()))?;
        let branch = args
            .get("branch")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("pr-{number}"));
        let main = facts.main_root();
        let at = path_for(
            &main,
            &facts.repo_name(),
            &branch,
            worktree_folder(cx).as_deref(),
        );
        // A detached worktree first ("HEAD" is no branch name): `gh` then makes the branch,
        // from a fork too.
        self.vcs
            .worktree_checkout(&facts.root, "HEAD", &at)
            .map_err(failed)?;
        if let Err(e) = forge.checkout(&at, number) {
            // Only the worktree just made goes; no branch was made.
            if let Err(rm) = self.vcs.worktree_remove(&main, &at, true) {
                eprintln!(
                    "terminescu: worktree {} left after a failed checkout: {rm}",
                    at.display()
                );
            }
            return Err(PluginError::Failed(e.to_string()));
        }
        self.vcs.forget(&main);
        // The branch `gh` made, which can differ from the one asked for (a fork's).
        let made = self
            .vcs
            .worktrees(&main)
            .ok()
            .and_then(|l| l.into_iter().find(|w| same_path(&w.path, &at))?.branch);
        let mut mark = json!({ "worktree": at, "repo": main });
        if let Some(b) = made {
            mark["branch"] = json!(b);
        }
        let title = format!("#{number} {branch}");
        let row = self.marked_row(&at_pane, &title, &at, &main, mark, cx)?;
        Ok(json!({ "row": row, "path": at }))
    }

    fn worktrees(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (_, facts) = self.pane_facts(args, cx)?;
        let list = self.vcs.worktrees(&facts.root).map_err(failed)?;
        Ok(json!(list))
    }

    /// A row for each of the repository's worktrees that no row shows yet, below the pane's
    /// row in git's order. A row shows a worktree when one of its shells is in it or the row is
    /// marked for it. Each row but the main working tree's is marked, so its close asks.
    fn open_worktrees(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (at_pane, facts) = self.pane_facts(args, cx)?;
        let main = facts.main_root();
        let list: Vec<_> = self
            .vcs
            .worktrees(&facts.root)
            .map_err(failed)?
            .into_iter()
            .filter(|w| !w.bare)
            .collect();
        let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let trees: Vec<PathBuf> = list.iter().map(|w| real(&w.path)).collect();
        // A folder lies in the worktree with the longest path it starts with, so a worktree
        // inside the main one counts as itself.
        let owner = |dir: &Path| {
            trees
                .iter()
                .filter(|t| dir.starts_with(t))
                .max_by_key(|t| t.as_os_str().len())
                .cloned()
        };
        let panes = cx.panes();
        let mut shown: HashSet<PathBuf> = panes
            .iter()
            .filter_map(|p| p.cwd.as_deref())
            .filter_map(|cwd| owner(&real(Path::new(cwd))))
            .collect();
        let mut rows: HashSet<Id> = panes.iter().map(|p| p.side_tab.clone()).collect();
        rows.extend(cx.side_tabs(&at_pane.window));
        for row in &rows {
            let marked = cx.row_data(row, WORKTREE_ROW);
            if let Some(w) = marked.as_ref().and_then(|m| m.get("worktree")?.as_str()) {
                shown.insert(real(Path::new(w)));
            }
        }
        let mut after = at_pane.side_tab.clone();
        let mut opened = Vec::new();
        for (w, tree) in list.iter().zip(&trees) {
            if shown.contains(tree) || !tree.is_dir() {
                continue;
            }
            let title = w.branch.clone().unwrap_or_else(|| {
                let name = tree.file_name().map(|n| n.to_string_lossy().into_owned());
                name.unwrap_or_else(|| tree.to_string_lossy().into_owned())
            });
            let cwd = tree.to_string_lossy();
            let row = cx.new_row(&at_pane.window, Some(&after), &title, &cwd)?;
            if !w.main {
                let mut mark = json!({ "worktree": tree, "repo": main });
                if let Some(b) = &w.branch {
                    mark["branch"] = json!(b);
                }
                cx.set_row_data(&row, WORKTREE_ROW, Some(mark))?;
            }
            opened.push(json!({ "row": row, "path": tree, "title": title }));
            after = row;
        }
        Ok(json!({ "opened": opened }))
    }

    /// The row's worktree mark and how many files changed in it; null for an unmarked row.
    /// `changed` is null when git cannot read the worktree, as after it was deleted by hand.
    fn row_worktree(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let row = arg(args, "row")?;
        let Some(mark) = cx.row_data(row, WORKTREE_ROW) else {
            return Ok(Value::Null);
        };
        // A broken mark never keeps a row open (D5): the row closes as an unmarked one.
        let Some(worktree) = mark.get("worktree").and_then(Value::as_str) else {
            eprintln!("terminescu: worktree mark of row {row} is broken");
            return Ok(Value::Null);
        };
        let changed = self.vcs.changes(Path::new(worktree)).map(|c| c.len()).ok();
        Ok(json!({ "worktree": worktree, "repo": mark.get("repo"), "changed": changed }))
    }

    /// What Settings › Git shows: the worktree folder, the GitHub switch and whether `gh` is
    /// on, with the reason when it is not.
    fn settings(&self, cx: &dyn PluginCx) -> Value {
        let folder = cx
            .setting(WORKTREE_FOLDER)
            .and_then(|v| v.as_str().map(String::from));
        let st = lock(&self.state);
        json!({
            "worktreeFolder": folder,
            "github": cx.setting(GITHUB) != Some(Value::Bool(false)),
            "githubOn": st.github,
            "githubOff": st.github_off,
            "version": st.version,
        })
    }

    /// Saves one setting from Settings › Git and answers the settings as they now are. A blank
    /// folder puts worktrees next to the repository again; GitHub on or off checks `gh` again.
    fn set_setting(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match arg(args, "key")? {
            WORKTREE_FOLDER => {
                let folder = match args.get("value") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(s)) if s.trim().is_empty() => None,
                    Some(Value::String(s)) => {
                        let s = s.trim();
                        if !(s == "~" || s.starts_with("~/") || Path::new(s).is_absolute()) {
                            return Err(PluginError::BadArgs(
                                "the folder is a full path, or starts with ~/".into(),
                            ));
                        }
                        Some(json!(s))
                    }
                    Some(_) => return Err(PluginError::BadArgs("the folder is text".into())),
                };
                cx.set_setting(WORKTREE_FOLDER, folder)?;
            }
            GITHUB => {
                let on = args
                    .get("value")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| PluginError::BadArgs("github is true or false".into()))?;
                // On is the default, so it is kept as no setting at all.
                cx.set_setting(GITHUB, (!on).then_some(json!(false)))?;
                self.set_github(cx);
            }
            other => return Err(PluginError::BadArgs(format!("no setting {other}"))),
        }
        Ok(self.settings(cx))
    }

    /// Only for a row this plugin marked, never with a branch delete.
    fn worktree_remove(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let row = arg(args, "row")?;
        let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
        let mark = cx
            .row_data(row, WORKTREE_ROW)
            .ok_or_else(|| PluginError::BadArgs("the row is not a worktree row".into()))?;
        let path = |k: &str| {
            mark.get(k)
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .ok_or_else(|| PluginError::Failed("the row's worktree mark is broken".into()))
        };
        let (worktree, repo) = (path("worktree")?, path("repo")?);
        let list = self.vcs.worktrees(&repo).map_err(failed)?;
        let listed = list.iter().find(|w| same_path(&w.path, &worktree));
        let marked = mark.get("branch").and_then(Value::as_str);
        // The mark may be older than what is there now: a worktree removed by hand and a new
        // one made at the same path. Then the row lets go of it and nothing is removed.
        let stale = match listed {
            None => Some("git no longer lists it as a worktree of the repository"),
            Some(w) if w.main || w.bare => Some("it is the repository's main working tree"),
            Some(w) if marked.is_some() && w.branch.as_deref() != marked => {
                Some("it now holds another branch")
            }
            Some(_) => None,
        };
        if let Some(why) = stale {
            cx.set_row_data(row, WORKTREE_ROW, None)?;
            return Err(PluginError::Failed(format!(
                "{} was not removed: {why}. The row no longer asks about it.",
                worktree.display()
            )));
        }
        match self.vcs.worktree_remove(&repo, &worktree, force) {
            Ok(()) => {
                self.vcs.forget(&repo);
                cx.set_row_data(row, WORKTREE_ROW, None)?;
                Ok(json!({ "removed": true }))
            }
            Err(VcsError::HasChanges) => {
                let changed = self.vcs.changes(&worktree).map(|c| c.len()).ok();
                Ok(json!({ "removed": false, "changed": changed }))
            }
            Err(e) => Err(failed(e)),
        }
    }
}

struct Located {
    side_tab: Id,
    window: Id,
    cwd: Option<String>,
}

/// Two paths name one folder, links resolved where the folders exist.
fn same_path(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    a == b || real(a) == real(b)
}

fn arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, PluginError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| PluginError::BadArgs(format!("{key} is missing")))
}

fn failed(e: VcsError) -> PluginError {
    match e {
        VcsError::Timeout => PluginError::Timeout,
        e => PluginError::Failed(e.to_string()),
    }
}

/// The folder from Settings › Git, with `~` read as the home folder. A relative one is ignored.
fn worktree_folder(cx: &dyn PluginCx) -> Option<PathBuf> {
    let set = cx.setting(WORKTREE_FOLDER)?;
    let s = set.as_str()?.trim();
    let path = match s.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None if s == "~" => PathBuf::from(std::env::var_os("HOME")?),
        None => PathBuf::from(s),
    };
    path.is_absolute().then(|| crate::cli::clean(&path))
}

/// The branch chip: the branch, a dot when files changed, `↑n` and `↓n`.
pub fn chip(tab: &str, f: &RepoFacts) -> Decoration {
    let mut text = f.head.label().to_string();
    if f.changed > 0 {
        text.push_str(" •");
    }
    if f.ahead > 0 {
        text.push_str(&format!(" ↑{}", f.ahead));
    }
    if f.behind > 0 {
        text.push_str(&format!(" ↓{}", f.behind));
    }
    let mut tip = Vec::new();
    let tone = match (f.op, f.conflicted) {
        (Some(op), n) if n > 0 => {
            tip.push(format!("{} stopped on a conflict", op.name()));
            Tone::Bad
        }
        (None, n) if n > 0 => {
            tip.push(format!("{n} unresolved conflicts"));
            Tone::Bad
        }
        (Some(op), _) => {
            tip.push(format!("{} in progress", op.name()));
            Tone::Warn
        }
        (None, _) => Tone::Neutral,
    };
    if let crate::Head::Detached(c) = &f.head {
        tip.push(format!("Detached at {c}"));
    }
    match &f.upstream {
        Some(u) => tip.push(format!("{u}: {} ahead, {} behind", f.ahead, f.behind)),
        None if f.head.branch().is_some() => tip.push("No upstream".into()),
        None => {}
    }
    tip.push(match f.changed {
        0 => "No changes".into(),
        1 => "1 changed file".into(),
        n => format!("{n} changed files"),
    });
    Decoration {
        target: Target::Tab(tab.into()),
        slot: Slot::Chip,
        key: "branch".into(),
        text,
        tone,
        tip: Some(tip.join(" · ")),
    }
}

/// The branch the facts are on, as the pull request cache keys it.
fn branch_of(f: &RepoFacts) -> Option<Branch> {
    Some((f.root.clone(), f.head.branch()?.to_string()))
}

/// The pull request chip: its number, toned by its checks; the tip names failing checks.
pub fn pr_chip(tab: &str, pr: &PullRequest) -> Decoration {
    let (tone, state) = match &pr.checks {
        Checks::None => (Tone::Neutral, "No checks".to_string()),
        Checks::Passing => (Tone::Good, "Checks passing".to_string()),
        Checks::Running => (Tone::Warn, "Checks running".to_string()),
        Checks::Failing(names) => (Tone::Bad, format!("Failing: {}", names.join(", "))),
    };
    Decoration {
        target: Target::Tab(tab.into()),
        slot: Slot::Chip,
        key: "pr".into(),
        text: format!("#{}", pr.number),
        tone,
        tip: Some(format!("{} · {state}", pr.title)),
    }
}

impl GitPlugin {
    /// What a tab shows; `None` facts clear it. `github` says a pull request answer counts:
    /// without it the checks fact is unknown rather than false. Only what differs from the
    /// last publish goes out, so chips never blink (D2).
    fn publish(
        &self,
        cx: &dyn PluginCx,
        tab: &str,
        facts: Option<&RepoFacts>,
        pr: Option<&PullRequest>,
        github: bool,
    ) {
        let failed = pr.map(|p| matches!(p.checks, Checks::Failing(_)));
        let failed = failed.or((github && facts.is_some()).then_some(false));
        let next = Shown {
            branch: facts.map(|f| chip(tab, f)),
            pr: pr.map(|p| pr_chip(tab, p)),
            facts: [
                facts.map(|f| Value::Bool(f.pushed())),
                facts.map(|f| Value::Bool(f.conflicted > 0)),
                failed.map(Value::Bool),
            ],
        };
        let mut shown = lock(&self.shown);
        let before = shown.remove(tab).unwrap_or_default();
        let target = Target::Tab(tab.into());
        // No call takes one chip away: a chip that goes clears the tab, and the rest is set
        // again.
        let gone = (before.branch.is_some() && next.branch.is_none())
            || (before.pr.is_some() && next.pr.is_none());
        if gone {
            cx.clear_decorations(Some(&target));
        }
        for (was, now) in [(&before.branch, &next.branch), (&before.pr, &next.pr)] {
            if let Some(d) = now
                && (gone || was.as_ref() != Some(d))
            {
                cx.set_decoration(d.clone());
            }
        }
        for (i, name) in TAB_FACTS.iter().enumerate() {
            if before.facts[i] != next.facts[i] {
                cx.set_fact(&target, name, next.facts[i].clone());
            }
        }
        if next != Shown::default() {
            shown.insert(tab.into(), next);
        }
    }

    /// Removes a worktree whose row has gone, after the orphan notice (D6). Checked against
    /// git like `worktreeRemove`, and never forced unless asked.
    fn worktree_remove_at(&self, args: &Value) -> Result<Value, PluginError> {
        let worktree = PathBuf::from(arg(args, "worktree")?);
        let repo = PathBuf::from(arg(args, "repo")?);
        let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
        let marked = args.get("branch").and_then(Value::as_str);
        let list = self.vcs.worktrees(&repo).map_err(failed)?;
        let refuse = |why: &str| {
            Err(PluginError::Failed(format!(
                "{} was not removed: {why}",
                worktree.display()
            )))
        };
        match list.iter().find(|w| same_path(&w.path, &worktree)) {
            None => return Ok(json!({ "removed": true, "gone": true })),
            Some(w) if w.main || w.bare => {
                return refuse("it is the repository's main working tree");
            }
            Some(w) if marked.is_some() && w.branch.as_deref() != marked => {
                return refuse("it now holds another branch");
            }
            Some(_) => {}
        }
        match self.vcs.worktree_remove(&repo, &worktree, force) {
            Ok(()) => {
                self.vcs.forget(&repo);
                Ok(json!({ "removed": true }))
            }
            Err(VcsError::HasChanges) => {
                let changed = self.vcs.changes(&worktree).map(|c| c.len()).ok();
                Ok(json!({ "removed": false, "changed": changed }))
            }
            Err(e) => Err(failed(e)),
        }
    }
}

impl Plugin for GitPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            // Launch reads the git version and `gh auth status` (3 s at most); other events
            // take a git read of a second at most. The longest call, prCheckout, runs
            // `worktree add`, `gh pr checkout` and on failure `worktree remove`, each
            // killed at CHANGE_LIMIT, so the budget never ends a call that still works.
            event_budget: Duration::from_secs(5),
            call_budget: crate::cli::CHANGE_LIMIT * 3 + Duration::from_secs(10),
            ..Manifest::new(
                "git",
                "Git",
                "Branch and changes on each tab, diffs, and worktree rows.",
            )
        }
    }

    fn on_event(&self, event: &CoreEvent, cx: &dyn PluginCx) {
        match event {
            CoreEvent::AppLaunched => self.launch(cx),
            CoreEvent::PaneSpawned { pane, cwd, .. } => self.touch(pane, cwd.as_deref(), cx),
            CoreEvent::CwdMoved { pane, cwd } => self.touch(pane, Some(cwd), cx),
            CoreEvent::CommandEnded { pane, .. } => {
                let cwd = lock(&self.state).panes.get_mut(pane).and_then(|p| {
                    p.pr_due = true;
                    p.cwd.clone()
                });
                if let Some(cwd) = cwd {
                    self.vcs.forget(Path::new(&cwd));
                }
                self.touch(pane, None, cx);
            }
            CoreEvent::PaneClosed { pane } => self.pane_closed(pane, cx),
            CoreEvent::TabClosed { tab } => self.tab_closed(tab, cx),
            CoreEvent::SideTabClosed { side_tab, data } => self.side_tab_closed(side_tab, data, cx),
            CoreEvent::SideTabSelected { side_tab, .. } => self.selected(side_tab, cx),
            CoreEvent::Timer { token } => {
                if token == VERSION_TIMER {
                    self.check_version(cx);
                } else if let Some(pane) = token.strip_prefix(TIMER) {
                    self.refresh(pane, cx);
                } else if let Some(pane) = token.strip_prefix(PR_READY) {
                    self.pr_ready(pane, cx);
                } else if let Some(pane) = token.strip_prefix(PR_TICK) {
                    self.tick(pane, cx);
                }
            }
            CoreEvent::CommandStarted { .. } | CoreEvent::Hook { .. } => {}
        }
    }

    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match method {
            "status" => {
                if args.get("recheck").and_then(Value::as_bool) == Some(true) {
                    self.set_github(cx);
                }
                let st = lock(&self.state);
                Ok(json!({
                    "version": st.version,
                    "stopped": st.stopped,
                    "github": st.github,
                    "githubOff": st.github_off,
                }))
            }
            "facts" => {
                let (_, facts) = self.pane_facts(&args, cx)?;
                let pr = self.known_pr(Some(&facts));
                Ok(json!({ "facts": facts, "pr": pr }))
            }
            "pullRequests" => self.pull_requests(&args, cx),
            "prCheckout" => self.pr_checkout(&args, cx),
            "changes" => self.changes(&args, cx),
            "baseText" => self.base_text(&args, cx),
            "worktreeAdd" => self.worktree_add(&args, cx),
            "worktreeWhere" => self.worktree_where(&args, cx),
            "worktreeRemoveAt" => self.worktree_remove_at(&args),
            "worktrees" => self.worktrees(&args, cx),
            "worktreeRemove" => self.worktree_remove(&args, cx),
            "openWorktrees" => self.open_worktrees(&args, cx),
            "rowWorktree" => self.row_worktree(&args, cx),
            "settings" => Ok(self.settings(cx)),
            "setSetting" => self.set_setting(&args, cx),
            other => Err(PluginError::UnknownMethod(other.into())),
        }
    }
}
