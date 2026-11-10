//! The git plugin (add-git-plugin D3, D5, D7). Shell events mark when a pane's repository may
//! have moved; each sets a 300 ms timer for the pane, and the timer reads the facts. The tab
//! shows the facts of its lead pane, the one that last moved or ran a command.

use crate::vcs::{DiffBase, RepoFacts, Vcs, VcsError};
use crate::worktree::path_for;
use crate::{CachedVcs, CoalescingVcs, GitCli, TimedVcs};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use terminescu_plugin_api::{
    CoreEvent, Decoration, Id, Manifest, PaneInfo, Plugin, PluginCx, PluginError, Slot, Target,
    Tone,
};

/// How long a pane waits after its last event before git runs.
pub const DEBOUNCE: Duration = Duration::from_millis(300);
/// The Changes section lists at most this many files, and the total.
pub const MAX_CHANGES: usize = 500;
/// Settings key: the folder worktrees go under; absent puts them next to the repository.
pub const WORKTREE_FOLDER: &str = "worktreeFolder";
/// Row data key: the worktree a row was made for, and its repository.
pub const WORKTREE_ROW: &str = "worktree";

const TIMER: &str = "pane:";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct GitPlugin {
    vcs: Arc<dyn Vcs>,
    /// Checked on launch; `None` in tests that bring their own provider.
    git: Option<GitCli>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    panes: HashMap<Id, PaneGit>,
    /// Per tab, the pane whose facts the tab shows.
    lead: HashMap<Id, Id>,
    /// Why the plugin stopped: no git, or one too old.
    stopped: Option<String>,
    version: Option<String>,
}

struct PaneGit {
    tab: Id,
    side_tab: Id,
    window: Id,
    cwd: Option<String>,
    facts: Option<RepoFacts>,
    /// Facts were read for the current cwd; until then `None` means unknown, not "no repo".
    read: bool,
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
            state: Mutex::default(),
        }
    }

    /// Reads through `vcs` and skips the version check.
    pub fn with_vcs(vcs: Arc<dyn Vcs>) -> Self {
        Self {
            vcs,
            git: None,
            state: Mutex::default(),
        }
    }

    fn launch(&self, cx: &dyn PluginCx) {
        cx.clear_decorations(None);
        let mut st = lock(&self.state);
        *st = State::default();
        if let Some(git) = &self.git {
            match git.version() {
                Ok(v) => st.version = Some(v),
                Err(e) => {
                    let why = e.to_string();
                    eprintln!("terminescu: git plugin stopped: {why}");
                    st.stopped = Some(why);
                    return;
                }
            }
        }
        let panes = cx.panes();
        for p in &panes {
            st.lead
                .entry(p.tab.clone())
                .or_insert_with(|| p.pane.clone());
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
            publish(cx, &tab, facts.as_ref());
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
        let tab = p.tab.clone();
        let leads = st.lead.get(&tab).map(String::as_str) == Some(pane);
        drop(st);
        let target = Target::Pane(pane.into());
        let f = facts.as_ref();
        cx.set_variable(
            &target,
            "branch",
            f.and_then(|f| f.head.branch().map(String::from)),
        );
        cx.set_variable(&target, "repo", f.map(RepoFacts::repo_name));
        if leads {
            publish(cx, &tab, f);
        }
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
        publish(cx, &gone.tab, next.as_ref().and_then(|n| n.1.as_ref()));
        if let Some((id, _)) = next {
            cx.schedule(Duration::ZERO, &format!("{TIMER}{id}"));
        }
    }

    fn tab_closed(&self, tab: &str, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
        st.lead.remove(tab);
        st.panes.retain(|_, p| p.tab != tab);
        drop(st);
        publish(cx, tab, None);
    }

    fn side_tab_closed(&self, side_tab: &str, cx: &dyn PluginCx) {
        let mut st = lock(&self.state);
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
            publish(cx, &t, None);
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

    fn base_text(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (_, facts) = self.pane_facts(args, cx)?;
        let path = arg(args, "path")?;
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
        let cwd = at.to_string_lossy();
        let row = cx.new_row(&at_pane.window, Some(&at_pane.side_tab), branch, &cwd)?;
        cx.set_row_data(
            &row,
            WORKTREE_ROW,
            Some(json!({ "worktree": at, "repo": main })),
        )?;
        Ok(json!({ "row": row, "path": at, "branch": branch, "base": base }))
    }

    fn worktrees(&self, args: &Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        let (_, facts) = self.pane_facts(args, cx)?;
        let list = self.vcs.worktrees(&facts.root).map_err(failed)?;
        Ok(json!(list))
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
    path.is_absolute().then_some(path)
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

/// What a tab shows for its lead pane's facts; `None` clears it.
fn publish(cx: &dyn PluginCx, tab: &str, facts: Option<&RepoFacts>) {
    let target = Target::Tab(tab.into());
    match facts {
        Some(f) => cx.set_decoration(chip(tab, f)),
        None => cx.clear_decorations(Some(&target)),
    }
    cx.set_fact(&target, "pushed", facts.map(|f| Value::Bool(f.pushed())));
    cx.set_fact(
        &target,
        "conflict",
        facts.map(|f| Value::Bool(f.conflicted > 0)),
    );
}

impl Plugin for GitPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            // One git read takes up to a second; a worktree checkout can take minutes.
            event_budget: Duration::from_secs(2),
            call_budget: Duration::from_secs(120),
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
                let cwd = lock(&self.state)
                    .panes
                    .get(pane)
                    .and_then(|p| p.cwd.clone());
                if let Some(cwd) = cwd {
                    self.vcs.forget(Path::new(&cwd));
                }
                self.touch(pane, None, cx);
            }
            CoreEvent::PaneClosed { pane } => self.pane_closed(pane, cx),
            CoreEvent::TabClosed { tab } => self.tab_closed(tab, cx),
            CoreEvent::SideTabClosed { side_tab } => self.side_tab_closed(side_tab, cx),
            CoreEvent::SideTabSelected { side_tab, .. } => self.selected(side_tab, cx),
            CoreEvent::Timer { token } => {
                if let Some(pane) = token.strip_prefix(TIMER) {
                    self.refresh(pane, cx);
                }
            }
            CoreEvent::CommandStarted { .. } | CoreEvent::Hook { .. } => {}
        }
    }

    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match method {
            "status" => {
                let st = lock(&self.state);
                Ok(json!({ "version": st.version, "stopped": st.stopped }))
            }
            "facts" => {
                let (_, facts) = self.pane_facts(&args, cx)?;
                Ok(json!(facts))
            }
            "changes" => self.changes(&args, cx),
            "baseText" => self.base_text(&args, cx),
            "worktreeAdd" => self.worktree_add(&args, cx),
            "worktrees" => self.worktrees(&args, cx),
            "worktreeRemove" => self.worktree_remove(&args, cx),
            other => Err(PluginError::UnknownMethod(other.into())),
        }
    }
}
