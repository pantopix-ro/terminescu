//! The git plugin over a fake facade and a fake provider (add-git-plugin 2.1), and once over
//! the real `git` for a worktree row.

mod common;

use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use terminescu_plugin_api::{
    CoreEvent, Decoration, Id, PaneInfo, Plugin, PluginCx, PluginError, Target, Tone,
};
use terminescu_plugin_git::plugin::{
    DEBOUNCE, MAX_BASE, MAX_CHANGES, VERSION_TIMER, WORKTREE_FOLDER, WORKTREE_ROW,
};
use terminescu_plugin_git::{
    Change, Checks, DiffBase, Forge, ForgeError, GitPlugin, Head, Op, PullRequest, RepoFacts, Vcs,
    VcsError, Worktree,
};

/// A new row: window, the row it goes after, title and cwd.
type NewRow = (String, Option<String>, String, String);

#[derive(Default)]
struct Cx {
    panes: Mutex<Vec<PaneInfo>>,
    chips: Mutex<HashMap<(Target, String), Decoration>>,
    facts: Mutex<HashMap<(Target, String), Value>>,
    vars: Mutex<HashMap<(Target, String), String>>,
    timers: Mutex<Vec<(Duration, String)>>,
    settings: Mutex<HashMap<String, Value>>,
    rows: Mutex<Vec<NewRow>>,
    row_data: Mutex<HashMap<(String, String), Value>>,
    emits: Mutex<Vec<(String, Value)>>,
    /// `new_row` fails, as when the window went meanwhile.
    no_rows: AtomicBool,
}

impl Cx {
    fn chip(&self, tab: &str) -> Option<Decoration> {
        let key = (Target::Tab(tab.into()), "branch".to_string());
        self.chips.lock().unwrap().get(&key).cloned()
    }
    fn fact(&self, tab: &str, name: &str) -> Option<Value> {
        let key = (Target::Tab(tab.into()), name.to_string());
        self.facts.lock().unwrap().get(&key).cloned()
    }
    fn var(&self, pane: &str, name: &str) -> Option<String> {
        let key = (Target::Pane(pane.into()), name.to_string());
        self.vars.lock().unwrap().get(&key).cloned()
    }
    fn timers(&self) -> Vec<(Duration, String)> {
        std::mem::take(&mut *self.timers.lock().unwrap())
    }
}

impl PluginCx for Cx {
    fn panes(&self) -> Vec<PaneInfo> {
        self.panes.lock().unwrap().clone()
    }
    fn workspace(&self) -> Value {
        Value::Null
    }
    /// The rows of the panes, then the rows `new_row` made.
    fn side_tabs(&self, _: &str) -> Vec<Id> {
        let mut rows: Vec<Id> = self.panes().into_iter().map(|p| p.side_tab).collect();
        rows.extend((0..self.rows.lock().unwrap().len()).map(|n| format!("s{}", n + 2)));
        rows
    }
    fn add_context_item(&self, _: &str, _: &str) -> Result<(), PluginError> {
        Ok(())
    }
    fn add_touched_file(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: bool,
    ) -> Result<(), PluginError> {
        Ok(())
    }
    fn set_decoration(&self, d: Decoration) {
        self.chips
            .lock()
            .unwrap()
            .insert((d.target.clone(), d.key.clone()), d);
    }
    fn clear_decorations(&self, target: Option<&Target>) {
        let mut chips = self.chips.lock().unwrap();
        match target {
            Some(t) => chips.retain(|k, _| &k.0 != t),
            None => chips.clear(),
        }
    }
    fn open_text(&self, _: &str, _: &str) -> Result<(), PluginError> {
        Ok(())
    }
    fn type_in_new_tab(&self, _: &str, _: &str, _: &str) -> Result<(), PluginError> {
        Ok(())
    }
    fn new_row(
        &self,
        window: &str,
        after: Option<&str>,
        title: &str,
        cwd: &str,
    ) -> Result<Id, PluginError> {
        if self.no_rows.load(Ordering::SeqCst) {
            return Err(PluginError::Failed("the window is gone".into()));
        }
        let mut rows = self.rows.lock().unwrap();
        rows.push((
            window.into(),
            after.map(String::from),
            title.into(),
            cwd.into(),
        ));
        Ok(format!("s{}", rows.len() + 1))
    }
    fn setting(&self, key: &str) -> Option<Value> {
        self.settings.lock().unwrap().get(key).cloned()
    }
    fn set_setting(&self, key: &str, value: Option<Value>) -> Result<(), PluginError> {
        let mut settings = self.settings.lock().unwrap();
        match value {
            Some(v) => settings.insert(key.into(), v),
            None => settings.remove(key),
        };
        Ok(())
    }
    fn row_data(&self, side_tab: &str, key: &str) -> Option<Value> {
        let k = (side_tab.to_string(), key.to_string());
        self.row_data.lock().unwrap().get(&k).cloned()
    }
    fn set_row_data(
        &self,
        side_tab: &str,
        key: &str,
        value: Option<Value>,
    ) -> Result<(), PluginError> {
        let k = (side_tab.to_string(), key.to_string());
        let mut data = self.row_data.lock().unwrap();
        match value {
            Some(v) => data.insert(k, v),
            None => data.remove(&k),
        };
        Ok(())
    }
    fn set_fact(&self, target: &Target, name: &str, value: Option<Value>) {
        let key = (target.clone(), name.to_string());
        let mut facts = self.facts.lock().unwrap();
        match value {
            Some(v) => facts.insert(key, v),
            None => facts.remove(&key),
        };
    }
    fn set_variable(&self, target: &Target, name: &str, value: Option<String>) {
        let key = (target.clone(), name.to_string());
        let mut vars = self.vars.lock().unwrap();
        match value {
            Some(v) => vars.insert(key, v),
            None => vars.remove(&key),
        };
    }
    fn emit(&self, event: &str, payload: Value) {
        self.emits.lock().unwrap().push((event.into(), payload));
    }
    fn schedule(&self, after: Duration, token: &str) {
        self.timers.lock().unwrap().push((after, token.into()));
    }
}

/// Answers per folder, as set by the test.
#[derive(Default)]
struct Fake {
    facts: Mutex<HashMap<PathBuf, Result<Option<RepoFacts>, VcsError>>>,
    reads: Mutex<Vec<PathBuf>>,
    forgot: Mutex<Vec<PathBuf>>,
    changes: Mutex<Vec<Change>>,
    base: Mutex<Option<Vec<u8>>>,
    added: Mutex<Vec<(PathBuf, String, String, PathBuf)>>,
    remove: Mutex<Option<Result<(), VcsError>>>,
    /// What `worktrees` lists, and each remove asked for: path and force.
    trees: Mutex<Vec<Worktree>>,
    removed: Mutex<Vec<(PathBuf, bool)>>,
}

impl Fake {
    fn set(&self, dir: &str, facts: Result<Option<RepoFacts>, VcsError>) {
        self.facts.lock().unwrap().insert(dir.into(), facts);
    }
}

impl Vcs for Fake {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError> {
        self.reads.lock().unwrap().push(dir.into());
        self.facts
            .lock()
            .unwrap()
            .get(dir)
            .cloned()
            .unwrap_or(Ok(None))
    }
    fn changes(&self, _: &Path) -> Result<Vec<Change>, VcsError> {
        Ok(self.changes.lock().unwrap().clone())
    }
    fn base_text(&self, _: &Path, _: &Path, _: DiffBase) -> Result<Option<Vec<u8>>, VcsError> {
        Ok(self.base.lock().unwrap().clone())
    }
    fn worktree_add(
        &self,
        root: &Path,
        branch: &str,
        base: &str,
        at: &Path,
    ) -> Result<(), VcsError> {
        self.added
            .lock()
            .unwrap()
            .push((root.into(), branch.into(), base.into(), at.into()));
        Ok(())
    }
    fn worktree_checkout(&self, _: &Path, _: &str, _: &Path) -> Result<(), VcsError> {
        Ok(())
    }
    fn worktrees(&self, _: &Path) -> Result<Vec<Worktree>, VcsError> {
        Ok(self.trees.lock().unwrap().clone())
    }
    fn worktree_remove(&self, _: &Path, at: &Path, force: bool) -> Result<(), VcsError> {
        self.removed.lock().unwrap().push((at.into(), force));
        self.remove.lock().unwrap().take().unwrap_or(Ok(()))
    }
    fn forget(&self, dir: &Path) {
        self.forgot.lock().unwrap().push(dir.into());
    }
}

fn repo(branch: &str) -> RepoFacts {
    RepoFacts {
        root: "/code/shop-api".into(),
        git_dir: "/code/shop-api/.git".into(),
        common_dir: "/code/shop-api/.git".into(),
        head: Head::Branch(branch.into()),
        upstream: Some(format!("origin/{branch}")),
        ahead: 0,
        behind: 0,
        changed: 0,
        conflicted: 0,
        op: None,
    }
}

fn pane(pane: &str, tab: &str, cwd: &str) -> PaneInfo {
    PaneInfo {
        pane: pane.into(),
        tab: tab.into(),
        side_tab: "s1".into(),
        window: "w1".into(),
        cwd: Some(cwd.into()),
    }
}

fn setup(panes: Vec<PaneInfo>) -> (GitPlugin, Arc<Fake>, Cx) {
    let fake = Arc::new(Fake::default());
    let plugin = GitPlugin::with_vcs(fake.clone());
    let cx = Cx::default();
    *cx.panes.lock().unwrap() = panes;
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    (plugin, fake, cx)
}

/// Runs every timer the plugin set, as the host would once they come due.
fn fire(plugin: &GitPlugin, cx: &Cx) {
    for (_, token) in cx.timers() {
        plugin.on_event(&CoreEvent::Timer { token }, cx);
    }
}

#[test]
fn launch_reads_each_pane_and_the_tab_shows_branch_and_ahead() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    let mut ahead = repo("feature/PROJ-482-retry");
    ahead.ahead = 2;
    fake.set("/code/shop-api", Ok(Some(ahead)));
    assert_eq!(cx.timers.lock().unwrap()[0].0, Duration::ZERO);
    fire(&plugin, &cx);
    let chip = cx.chip("t1").unwrap();
    assert_eq!(chip.text, "feature/PROJ-482-retry ↑2");
    assert_eq!(chip.tone, Tone::Neutral);
    assert_eq!(cx.fact("t1", "pushed"), Some(json!(false)));
    assert_eq!(cx.fact("t1", "conflict"), Some(json!(false)));
    assert_eq!(
        cx.var("p1", "branch").as_deref(),
        Some("feature/PROJ-482-retry")
    );
    assert_eq!(cx.var("p1", "repo").as_deref(), Some("shop-api"));
    // The Changes section hears that the pane's facts were read.
    assert!(
        cx.emits
            .lock()
            .unwrap()
            .contains(&("facts".into(), json!({ "pane": "p1" })))
    );
}

#[test]
fn a_cd_waits_for_the_debounce_and_reads_the_new_folder() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/tmp")]);
    fire(&plugin, &cx);
    assert!(cx.chip("t1").is_none());
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    for _ in 0..3 {
        plugin.on_event(
            &CoreEvent::CwdMoved {
                pane: "p1".into(),
                cwd: "/code/shop-api".into(),
            },
            &cx,
        );
    }
    let before = fake.reads.lock().unwrap().len();
    let timers = cx.timers();
    assert!(
        timers
            .iter()
            .all(|t| t == &(DEBOUNCE, "pane:p1".to_string()))
    );
    assert_eq!(
        fake.reads.lock().unwrap().len(),
        before,
        "no git before the timer"
    );
    // The host keeps one timer per token, so three moves run git once.
    plugin.on_event(
        &CoreEvent::Timer {
            token: timers[0].1.clone(),
        },
        &cx,
    );
    assert_eq!(cx.chip("t1").unwrap().text, "main");
    assert_eq!(cx.fact("t1", "pushed"), Some(json!(true)));
}

#[test]
fn a_stopped_rebase_turns_the_chip_red_and_says_so() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    let mut stopped = repo("feature/PROJ-482-retry");
    stopped.op = Some(Op::Rebase);
    stopped.conflicted = 1;
    stopped.changed = 1;
    fake.set("/code/shop-api", Ok(Some(stopped)));
    fire(&plugin, &cx);
    let chip = cx.chip("t1").unwrap();
    assert_eq!(chip.tone, Tone::Bad);
    assert_eq!(chip.text, "feature/PROJ-482-retry •");
    assert!(
        chip.tip
            .unwrap()
            .starts_with("Rebase stopped on a conflict")
    );
    assert_eq!(cx.fact("t1", "conflict"), Some(json!(true)));
}

#[test]
fn leaving_the_repository_clears_chip_facts_and_variables() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    fire(&plugin, &cx);
    assert!(cx.chip("t1").is_some());
    plugin.on_event(
        &CoreEvent::CwdMoved {
            pane: "p1".into(),
            cwd: "/tmp".into(),
        },
        &cx,
    );
    fire(&plugin, &cx);
    assert!(cx.chip("t1").is_none());
    assert_eq!(cx.fact("t1", "pushed"), None);
    assert_eq!(cx.var("p1", "branch"), None);
}

#[test]
fn a_slow_git_keeps_the_last_facts() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    fire(&plugin, &cx);
    fake.set("/code/shop-api", Err(VcsError::Timeout));
    plugin.on_event(
        &CoreEvent::CommandEnded {
            pane: "p1".into(),
            command: "git pull".into(),
            exit: Some(0),
        },
        &cx,
    );
    fire(&plugin, &cx);
    assert_eq!(cx.chip("t1").unwrap().text, "main");
    // A command that ended told the cache to look again.
    assert_eq!(
        fake.forgot.lock().unwrap().as_slice(),
        [PathBuf::from("/code/shop-api")]
    );
}

#[test]
fn without_git_the_plugin_stops_and_says_why() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Err(VcsError::NoGit));
    fire(&plugin, &cx);
    let status = plugin.call("status", Value::Null, &cx).unwrap();
    assert!(status["stopped"].as_str().is_some());
    plugin.on_event(
        &CoreEvent::CwdMoved {
            pane: "p1".into(),
            cwd: "/code".into(),
        },
        &cx,
    );
    assert!(cx.timers().is_empty());
    // Switching the plugin off and on (AppLaunched) tries again.
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    fire(&plugin, &cx);
    assert!(cx.chip("t1").is_some());
}

#[test]
fn the_tab_follows_the_pane_that_moved_last() {
    let (plugin, fake, cx) = setup(vec![
        pane("p1", "t1", "/code/shop-api"),
        pane("p2", "t1", "/code/shop-web"),
    ]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    let mut web = repo("develop");
    web.root = "/code/shop-web".into();
    fake.set("/code/shop-web", Ok(Some(web)));
    fire(&plugin, &cx);
    assert_eq!(
        cx.chip("t1").unwrap().text,
        "main",
        "the first pane leads at launch"
    );
    plugin.on_event(
        &CoreEvent::CommandEnded {
            pane: "p2".into(),
            command: "ls".into(),
            exit: Some(0),
        },
        &cx,
    );
    assert_eq!(
        cx.chip("t1").unwrap().text,
        "develop",
        "known facts show at once"
    );
    // The other pane's own timer does not take the chip back.
    plugin.on_event(
        &CoreEvent::Timer {
            token: "pane:p1".into(),
        },
        &cx,
    );
    assert_eq!(cx.chip("t1").unwrap().text, "develop");
    // Closing the lead hands the tab back.
    plugin.on_event(&CoreEvent::PaneClosed { pane: "p2".into() }, &cx);
    assert_eq!(cx.chip("t1").unwrap().text, "main");
    plugin.on_event(&CoreEvent::TabClosed { tab: "t1".into() }, &cx);
    assert!(cx.chip("t1").is_none());
}

#[test]
fn changes_list_the_first_500_and_the_total() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    let one = Change {
        path: "src/retry.ts".into(),
        from: None,
        staged: false,
        unstaged: true,
        untracked: false,
        conflicted: false,
        added: Some(12),
        removed: Some(3),
        binary: false,
    };
    *fake.changes.lock().unwrap() = vec![one; 620];
    let got = plugin
        .call("changes", json!({ "pane": "p1" }), &cx)
        .unwrap();
    assert_eq!(got["total"], json!(620));
    assert_eq!(got["changes"].as_array().unwrap().len(), MAX_CHANGES);
    assert_eq!(got["changes"][0]["added"], json!(12));
    assert!(matches!(
        plugin.call("changes", json!({}), &cx),
        Err(PluginError::BadArgs(_))
    ));
}

#[test]
fn base_text_is_text_or_a_binary_size() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    *fake.base.lock().unwrap() = Some(b"one\ntwo\n".to_vec());
    let args = json!({ "pane": "p1", "path": "a.txt", "base": "index" });
    let got = plugin.call("baseText", args, &cx).unwrap();
    assert_eq!(got["text"], json!("one\ntwo\n"));
    *fake.base.lock().unwrap() = Some(vec![0x89, b'P', 0, 1]);
    let got = plugin
        .call("baseText", json!({ "pane": "p1", "path": "logo.png" }), &cx)
        .unwrap();
    assert_eq!(
        (got["binary"].clone(), got["size"].clone()),
        (json!(true), json!(4))
    );
    *fake.base.lock().unwrap() = None;
    let got = plugin
        .call("baseText", json!({ "pane": "p1", "path": "new.ts" }), &cx)
        .unwrap();
    assert_eq!(got["exists"], json!(false));
    let bad = json!({ "pane": "p1", "path": "a.txt", "base": "theirs" });
    assert!(matches!(
        plugin.call("baseText", bad, &cx),
        Err(PluginError::BadArgs(_))
    ));
    // A diff tab names its working tree instead of a pane.
    *fake.base.lock().unwrap() = Some(b"one\n".to_vec());
    let by_root = json!({ "root": "/code/shop-api", "path": "a.txt" });
    assert_eq!(
        plugin.call("baseText", by_root, &cx).unwrap()["text"],
        json!("one\n")
    );
    assert!(matches!(
        plugin.call("baseText", json!({ "root": "code", "path": "a.txt" }), &cx),
        Err(PluginError::BadArgs(_))
    ));
    *fake.base.lock().unwrap() = Some(vec![b'a'; MAX_BASE + 1]);
    let got = plugin
        .call("baseText", json!({ "pane": "p1", "path": "big.sql" }), &cx)
        .unwrap();
    assert_eq!((got["large"].clone(), got.get("text")), (json!(true), None));
}

#[test]
fn a_worktree_row_goes_under_the_folder_below_the_row_and_is_marked() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api/src")]);
    fake.set("/code/shop-api/src", Ok(Some(repo("develop"))));
    cx.settings
        .lock()
        .unwrap()
        .insert(WORKTREE_FOLDER.into(), json!("/Users/owner/worktrees"));
    let got = plugin
        .call(
            "worktreeAdd",
            json!({ "pane": "p1", "branch": "PROJ-490-refunds" }),
            &cx,
        )
        .unwrap();
    let at = "/Users/owner/worktrees/shop-api/PROJ-490-refunds";
    assert_eq!(got["path"], json!(at));
    assert_eq!(
        got["base"],
        json!("develop"),
        "the current branch by default"
    );
    let added = fake.added.lock().unwrap()[0].clone();
    assert_eq!(added.1, "PROJ-490-refunds");
    let rows = cx.rows.lock().unwrap().clone();
    assert_eq!(
        rows,
        [(
            "w1".into(),
            Some("s1".into()),
            "PROJ-490-refunds".into(),
            at.into()
        )]
    );
    let row = got["row"].as_str().unwrap();
    assert_eq!(
        cx.row_data(row, WORKTREE_ROW),
        Some(json!({ "worktree": at, "repo": "/code/shop-api", "branch": "PROJ-490-refunds" }))
    );
}

#[test]
fn only_a_marked_row_removes_its_worktree_and_changes_ask_first() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    assert!(matches!(
        plugin.call("worktreeRemove", json!({ "row": "s1" }), &cx),
        Err(PluginError::BadArgs(_))
    ));
    let mark = json!({ "worktree": "/code/shop-api-x", "repo": "/code/shop-api", "branch": "x" });
    cx.set_row_data("s2", WORKTREE_ROW, Some(mark)).unwrap();
    *fake.trees.lock().unwrap() = vec![
        tree("/code/shop-api", "main", true),
        tree("/code/shop-api-x", "x", false),
    ];
    *fake.remove.lock().unwrap() = Some(Err(VcsError::HasChanges));
    let change = Change {
        path: "a".into(),
        from: None,
        staged: false,
        unstaged: true,
        untracked: false,
        conflicted: false,
        added: None,
        removed: None,
        binary: false,
    };
    *fake.changes.lock().unwrap() = vec![change; 2];
    let got = plugin
        .call("worktreeRemove", json!({ "row": "s2" }), &cx)
        .unwrap();
    assert_eq!(got, json!({ "removed": false, "changed": 2 }));
    assert!(cx.row_data("s2", WORKTREE_ROW).is_some());
    let got = plugin
        .call("worktreeRemove", json!({ "row": "s2", "force": true }), &cx)
        .unwrap();
    assert_eq!(got, json!({ "removed": true }));
    assert!(cx.row_data("s2", WORKTREE_ROW).is_none());
}

#[test]
fn a_new_worktree_row_from_a_real_repository() {
    let dir = common::repo("shop-api");
    let plugin = GitPlugin::with_vcs(Arc::new(common::git_cli()));
    let cx = Cx::default();
    *cx.panes.lock().unwrap() = vec![pane("p1", "t1", dir.to_str().unwrap())];
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    let got = plugin
        .call(
            "worktreeAdd",
            json!({ "pane": "p1", "branch": "PROJ-490-refunds" }),
            &cx,
        )
        .unwrap();
    let at = dir.parent().unwrap().join("shop-api-PROJ-490-refunds");
    assert_eq!(got["path"], json!(at));
    assert!(at.join("a.txt").is_file());
    let branch = common::git(&at, &["branch", "--show-current"]);
    assert_eq!(branch.trim(), "PROJ-490-refunds");
}

#[test]
fn open_worktrees_opens_a_row_for_each_one_no_row_shows_and_marks_it() {
    let dir = common::repo("shop-api");
    let parent = dir.parent().unwrap();
    for b in ["PROJ-490-refunds", "PROJ-491-tax"] {
        let at = parent.join(format!("shop-api-{b}"));
        common::git(&dir, &["worktree", "add", "-b", b, at.to_str().unwrap()]);
    }
    let plugin = GitPlugin::with_vcs(Arc::new(common::git_cli()));
    let cx = Cx::default();
    let tax = parent.join("shop-api-PROJ-491-tax");
    // The main tree's row, and a row whose shell is in a folder of the second worktree.
    *cx.panes.lock().unwrap() = vec![
        pane("p1", "t1", dir.to_str().unwrap()),
        PaneInfo {
            side_tab: "s9".into(),
            ..pane("p2", "t2", tax.join("src").to_str().unwrap())
        },
    ];
    std::fs::create_dir_all(tax.join("src")).unwrap();
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    let got = plugin
        .call("openWorktrees", json!({ "pane": "p1" }), &cx)
        .unwrap();
    let refunds = parent.join("shop-api-PROJ-490-refunds");
    assert_eq!(got["opened"].as_array().unwrap().len(), 1, "{got}");
    let rows = cx.rows.lock().unwrap().clone();
    assert_eq!(
        rows,
        [(
            "w1".into(),
            Some("s1".into()),
            "PROJ-490-refunds".into(),
            refunds.to_string_lossy().into_owned()
        )]
    );
    let row = got["opened"][0]["row"].as_str().unwrap();
    assert_eq!(
        cx.row_data(row, WORKTREE_ROW),
        Some(json!({ "worktree": refunds, "repo": dir, "branch": "PROJ-490-refunds" }))
    );
    // The new row's mark counts as showing it, though no shell of it is there yet.
    let again = plugin
        .call("openWorktrees", json!({ "pane": "p1" }), &cx)
        .unwrap();
    assert_eq!(again["opened"], json!([]));
}

#[test]
fn row_worktree_names_the_mark_and_counts_its_changes() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    assert_eq!(
        plugin
            .call("rowWorktree", json!({ "row": "s1" }), &cx)
            .unwrap(),
        Value::Null
    );
    let mark = json!({ "worktree": "/code/shop-api-x", "repo": "/code/shop-api" });
    cx.set_row_data("s2", WORKTREE_ROW, Some(mark)).unwrap();
    let change = Change {
        path: "a".into(),
        from: None,
        staged: false,
        unstaged: true,
        untracked: false,
        conflicted: false,
        added: None,
        removed: None,
        binary: false,
    };
    *fake.changes.lock().unwrap() = vec![change; 2];
    let got = plugin
        .call("rowWorktree", json!({ "row": "s2" }), &cx)
        .unwrap();
    assert_eq!(
        got,
        json!({ "worktree": "/code/shop-api-x", "repo": "/code/shop-api", "changed": 2 })
    );
}

/// A forge that answers one pull request per branch and counts its lookups.
#[derive(Default)]
struct FakeForge {
    prs: Mutex<HashMap<String, PullRequest>>,
    lookups: Mutex<usize>,
    out: bool,
    /// Keeps a lookup running until cleared.
    hold: AtomicBool,
    panics: AtomicBool,
}

impl Forge for FakeForge {
    fn available(&self) -> Result<(), ForgeError> {
        if self.out {
            Err(ForgeError::LoggedOut)
        } else {
            Ok(())
        }
    }
    fn pr_for(&self, _: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        *self.lookups.lock().unwrap() += 1;
        while self.hold.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        if self.panics.load(Ordering::SeqCst) {
            panic!("gh answered something unexpected");
        }
        Ok(self.prs.lock().unwrap().get(branch).cloned())
    }
    fn open_prs(&self, _: &Path) -> Result<Vec<PullRequest>, ForgeError> {
        Ok(self.prs.lock().unwrap().values().cloned().collect())
    }
    fn checkout(&self, _: &Path, _: u32) -> Result<(), ForgeError> {
        Ok(())
    }
}

fn pr(number: u32, branch: &str, checks: Checks) -> PullRequest {
    PullRequest {
        number,
        title: "Retry failed payments".into(),
        url: format!("https://github.com/acme/shop-api/pull/{number}"),
        branch: branch.into(),
        checks,
    }
}

fn with_forge(forge: Arc<FakeForge>) -> (GitPlugin, Arc<Fake>, Cx) {
    let fake = Arc::new(Fake::default());
    fake.set("/code/shop-api", Ok(Some(repo("feature/PROJ-482-retry"))));
    let plugin = GitPlugin::with_vcs(fake.clone()).with_forge(forge);
    let cx = Cx::default();
    *cx.panes.lock().unwrap() = vec![pane("p1", "t1", "/code/shop-api")];
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    (plugin, fake, cx)
}

/// Fires timers until the tab shows a pull request chip, as the host would.
fn until_pr(plugin: &GitPlugin, cx: &Cx) -> Option<Decoration> {
    for _ in 0..200 {
        let pr = cx
            .chips
            .lock()
            .unwrap()
            .values()
            .find(|d| d.key == "pr")
            .cloned();
        if pr.is_some() {
            return pr;
        }
        for (_, token) in cx.timers() {
            if !token.starts_with("prtick:") {
                plugin.on_event(&CoreEvent::Timer { token }, cx);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn a_failing_check_shows_the_pr_in_red_and_sets_the_fact() {
    let forge = Arc::new(FakeForge::default());
    forge.prs.lock().unwrap().insert(
        "feature/PROJ-482-retry".into(),
        pr(
            918,
            "feature/PROJ-482-retry",
            Checks::Failing(vec!["lint".into()]),
        ),
    );
    let (plugin, _, cx) = with_forge(forge.clone());
    let chip = until_pr(&plugin, &cx).expect("a pr chip");
    assert_eq!(chip.text, "#918");
    assert_eq!(chip.tone, Tone::Bad);
    assert!(chip.tip.unwrap().contains("Failing: lint"));
    assert_eq!(cx.fact("t1", "checksFailed"), Some(json!(true)));
    assert_eq!(cx.var("p1", "pr").as_deref(), Some("918"));
    // The branch chip stays beside it.
    assert_eq!(cx.chip("t1").unwrap().key, "branch");
    let status = plugin.call("status", Value::Null, &cx).unwrap();
    assert_eq!(status["github"], json!(true));
}

/// Fires timers until no pull request lookup is waited on, as the host would.
fn settle(plugin: &GitPlugin, cx: &Cx) {
    for _ in 0..200 {
        let timers = cx.timers();
        if timers.iter().all(|t| !t.1.starts_with("prready:")) {
            return;
        }
        for (_, token) in timers {
            if !token.starts_with("prtick:") {
                plugin.on_event(&CoreEvent::Timer { token }, cx);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("a lookup never settled");
}

fn command_ended(plugin: &GitPlugin, cx: &Cx) {
    plugin.on_event(
        &CoreEvent::CommandEnded {
            pane: "p1".into(),
            command: "git push".into(),
            exit: Some(0),
        },
        cx,
    );
}

#[test]
fn a_command_ending_during_a_lookup_looks_again_after_it() {
    let forge = Arc::new(FakeForge::default());
    forge.hold.store(true, Ordering::SeqCst);
    let (plugin, _, cx) = with_forge(forge.clone());
    fire(&plugin, &cx);
    // The lookup thread is held inside `gh`.
    while *forge.lookups.lock().unwrap() == 0 {
        std::thread::sleep(Duration::from_millis(2));
    }
    command_ended(&plugin, &cx);
    fire(&plugin, &cx);
    assert_eq!(*forge.lookups.lock().unwrap(), 1, "one runs at a time");
    forge.prs.lock().unwrap().insert(
        "feature/PROJ-482-retry".into(),
        pr(918, "feature/PROJ-482-retry", Checks::Passing),
    );
    forge.hold.store(false, Ordering::SeqCst);
    settle(&plugin, &cx);
    assert_eq!(*forge.lookups.lock().unwrap(), 2);
    assert_eq!(cx.var("p1", "pr").as_deref(), Some("918"));
}

#[test]
fn a_panicking_lookup_does_not_block_the_next() {
    let forge = Arc::new(FakeForge::default());
    forge.panics.store(true, Ordering::SeqCst);
    let (plugin, _, cx) = with_forge(forge.clone());
    fire(&plugin, &cx);
    settle(&plugin, &cx);
    forge.panics.store(false, Ordering::SeqCst);
    command_ended(&plugin, &cx);
    fire(&plugin, &cx);
    settle(&plugin, &cx);
    assert_eq!(*forge.lookups.lock().unwrap(), 2);
}

#[test]
fn the_minute_tick_goes_on_when_its_git_read_times_out() {
    let forge = Arc::new(FakeForge::default());
    let (plugin, fake, cx) = with_forge(forge);
    fire(&plugin, &cx);
    settle(&plugin, &cx);
    fake.set("/code/shop-api", Err(VcsError::Timeout));
    plugin.on_event(
        &CoreEvent::Timer {
            token: "prtick:p1".into(),
        },
        &cx,
    );
    assert!(
        cx.timers()
            .contains(&(Duration::from_secs(60), "prtick:p1".into()))
    );
}

#[test]
fn a_lookup_runs_once_a_minute_unless_a_command_ended() {
    let forge = Arc::new(FakeForge::default());
    let (plugin, _, cx) = with_forge(forge.clone());
    fire(&plugin, &cx);
    let settled = || {
        for _ in 0..200 {
            if cx
                .timers
                .lock()
                .unwrap()
                .iter()
                .all(|t| !t.1.starts_with("prready:"))
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
            fire(&plugin, &cx);
        }
    };
    settled();
    assert_eq!(*forge.lookups.lock().unwrap(), 1);
    // No pull request: the fact says so instead of staying unknown.
    assert_eq!(cx.fact("t1", "checksFailed"), Some(json!(false)));
    plugin.on_event(
        &CoreEvent::CwdMoved {
            pane: "p1".into(),
            cwd: "/code/shop-api".into(),
        },
        &cx,
    );
    fire(&plugin, &cx);
    settled();
    assert_eq!(*forge.lookups.lock().unwrap(), 1, "a recent answer stands");
    plugin.on_event(
        &CoreEvent::CommandEnded {
            pane: "p1".into(),
            command: "gh pr create".into(),
            exit: Some(0),
        },
        &cx,
    );
    fire(&plugin, &cx);
    settled();
    assert_eq!(*forge.lookups.lock().unwrap(), 2);
}

#[test]
fn without_gh_there_is_no_pr_chip_and_no_pr_calls() {
    let forge = Arc::new(FakeForge {
        out: true,
        ..FakeForge::default()
    });
    let (plugin, _, cx) = with_forge(forge.clone());
    fire(&plugin, &cx);
    assert!(cx.chips.lock().unwrap().values().all(|d| d.key != "pr"));
    assert_eq!(cx.fact("t1", "checksFailed"), None);
    assert_eq!(*forge.lookups.lock().unwrap(), 0);
    let status = plugin.call("status", Value::Null, &cx).unwrap();
    assert_eq!(status["github"], json!(false));
    assert!(status["githubOff"].as_str().unwrap().contains("logged in"));
    assert!(
        plugin
            .call("pullRequests", json!({ "pane": "p1" }), &cx)
            .is_err()
    );
}

#[test]
fn the_github_setting_turns_the_part_off() {
    let forge = Arc::new(FakeForge::default());
    let (plugin, _, cx) = with_forge(forge.clone());
    cx.settings
        .lock()
        .unwrap()
        .insert("github".into(), json!(false));
    let status = plugin
        .call("status", json!({ "recheck": true }), &cx)
        .unwrap();
    assert_eq!(status["github"], json!(false));
    assert_eq!(status["githubOff"], json!("GitHub is off in Settings"));
    // The UI half hears each switch: on at launch, off after the recheck.
    let told: Vec<_> = cx
        .emits
        .lock()
        .unwrap()
        .iter()
        .filter(|(e, _)| e == "github")
        .map(|(_, p)| p["on"].clone())
        .collect();
    assert_eq!(told, [json!(true), json!(false)]);
}

#[test]
fn settings_keep_the_worktree_folder_and_switch_github() {
    let forge = Arc::new(FakeForge::default());
    let (plugin, _, cx) = with_forge(forge);
    let got = plugin.call("settings", Value::Null, &cx).unwrap();
    assert_eq!(
        (&got["worktreeFolder"], &got["github"], &got["githubOn"]),
        (&Value::Null, &json!(true), &json!(true))
    );
    let set = |key: &str, value: Value| {
        plugin.call("setSetting", json!({ "key": key, "value": value }), &cx)
    };
    let got = set(WORKTREE_FOLDER, json!(" ~/worktrees ")).unwrap();
    assert_eq!(got["worktreeFolder"], json!("~/worktrees"));
    assert!(matches!(
        set(WORKTREE_FOLDER, json!("worktrees")),
        Err(PluginError::BadArgs(_))
    ));
    assert_eq!(
        cx.setting(WORKTREE_FOLDER),
        Some(json!("~/worktrees")),
        "a refused folder keeps the last one"
    );
    let got = set(WORKTREE_FOLDER, json!("")).unwrap();
    assert_eq!(got["worktreeFolder"], Value::Null);
    assert_eq!(cx.setting(WORKTREE_FOLDER), None);
    // GitHub off: kept as false, the part goes off at once and the UI half hears it.
    let got = set("github", json!(false)).unwrap();
    assert_eq!(
        (&got["github"], &got["githubOn"]),
        (&json!(false), &json!(false))
    );
    assert_eq!(got["githubOff"], json!("GitHub is off in Settings"));
    let last = |cx: &Cx| {
        let emits = cx.emits.lock().unwrap();
        emits
            .iter()
            .rev()
            .find(|(e, _)| e == "github")
            .map(|(_, p)| p["on"].clone())
    };
    assert_eq!(last(&cx), Some(json!(false)));
    let got = set("github", json!(true)).unwrap();
    assert_eq!(got["githubOn"], json!(true));
    assert_eq!(
        cx.setting("github"),
        None,
        "on is the default, kept as nothing"
    );
    assert_eq!(last(&cx), Some(json!(true)));
    assert!(matches!(
        set("colour", json!("red")),
        Err(PluginError::BadArgs(_))
    ));
}

fn tree(path: &str, branch: &str, main: bool) -> Worktree {
    Worktree {
        path: path.into(),
        branch: Some(branch.into()),
        head: None,
        bare: false,
        main,
    }
}

#[test]
fn a_stale_mark_removes_nothing_and_lets_go() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    *fake.trees.lock().unwrap() = vec![
        tree("/code/shop-api", "main", true),
        tree("/code/shop-api-x", "PROJ-7-other", false),
    ];
    let cases = [
        // A mark on the main working tree.
        json!({ "worktree": "/code/shop-api", "repo": "/code/shop-api" }),
        // A path git does not list.
        json!({ "worktree": "/code/elsewhere", "repo": "/code/shop-api" }),
        // The path was reused for another branch since the row was made.
        json!({ "worktree": "/code/shop-api-x", "repo": "/code/shop-api", "branch": "x" }),
    ];
    for mark in cases {
        cx.set_row_data("s2", WORKTREE_ROW, Some(mark.clone()))
            .unwrap();
        let got = plugin.call("worktreeRemove", json!({ "row": "s2", "force": true }), &cx);
        assert!(
            matches!(&got, Err(PluginError::Failed(m)) if m.contains("was not removed")),
            "{mark}: {got:?}"
        );
        assert!(cx.row_data("s2", WORKTREE_ROW).is_none(), "{mark}");
    }
    assert!(fake.removed.lock().unwrap().is_empty());
    // An old mark with no branch still removes a listed worktree.
    let old = json!({ "worktree": "/code/shop-api-x", "repo": "/code/shop-api" });
    cx.set_row_data("s2", WORKTREE_ROW, Some(old)).unwrap();
    let got = plugin
        .call("worktreeRemove", json!({ "row": "s2" }), &cx)
        .unwrap();
    assert_eq!(got, json!({ "removed": true }));
}

#[test]
fn a_worktree_with_no_row_goes_again_and_the_error_says_so() {
    let (plugin, fake, cx) = setup(vec![pane("p1", "t1", "/code/shop-api")]);
    fake.set("/code/shop-api", Ok(Some(repo("main"))));
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    cx.no_rows.store(true, Ordering::SeqCst);
    let got = plugin.call(
        "worktreeAdd",
        json!({ "pane": "p1", "branch": "PROJ-490-refunds" }),
        &cx,
    );
    let Err(PluginError::Failed(m)) = got else {
        panic!("{got:?}")
    };
    assert!(
        m.contains("the window is gone") && m.contains("removed again"),
        "{m}"
    );
    let removed = fake.removed.lock().unwrap().clone();
    assert_eq!(
        removed,
        [(PathBuf::from("/code/shop-api-PROJ-490-refunds"), false)]
    );
}

#[test]
fn a_slow_git_version_at_launch_asks_again_instead_of_stopping() {
    let dir = common::folder("slow-version");
    let marker = dir.join("asked");
    let script = dir.join("git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ ! -e '{m}' ]; then touch '{m}'; sleep 5; fi\necho 'git version 2.44.0'\n",
            m = marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    // The second ask must fit in the limit too, and macOS scanning a fresh script on a loaded CI
    // machine can take over a second: keep the limit well above that and the first answer above it.
    let git = terminescu_plugin_git::GitCli::new(&script).with_limit(Duration::from_secs(3));
    let plugin = GitPlugin::with_vcs(Arc::new(Fake::default())).with_git(git);
    let cx = Cx::default();
    plugin.on_event(&CoreEvent::AppLaunched, &cx);
    let st = plugin.call("status", json!({}), &cx).unwrap();
    assert_eq!(st["stopped"], Value::Null, "{st}");
    assert_eq!(st["version"], Value::Null);
    assert!(cx.timers().iter().any(|(_, t)| t == VERSION_TIMER));
    plugin.on_event(
        &CoreEvent::Timer {
            token: VERSION_TIMER.into(),
        },
        &cx,
    );
    let st = plugin.call("status", json!({}), &cx).unwrap();
    assert_eq!(st["version"], json!("2.44.0"), "{st}");
}
