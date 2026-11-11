//! Baselines against a real git (add-claude-insights 1.1, 1.3, 1.4): the snapshot leaves the
//! repository as it was, changes list Bash and formatter edits in a repository and only edit
//! tool edits outside one, and retention keeps running sessions.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use terminescu_plugin_api::baseline::{self as store, SIZE_CAP, first_touch};
use terminescu_plugin_claude::baseline::{Meta, Store};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tsc-claude-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The temp folder sits behind a /var -> /private/var link; git answers with the real path.
    dir.canonicalize().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        // `git status` would otherwise refresh the index it is asked to compare.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// `shop-api` with one commit, an uncommitted edit, an untracked file and an ignored one.
fn repo(name: &str) -> (PathBuf, PathBuf) {
    let base = folder(name);
    let repo = base.join("shop-api");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(repo.join("src/retry.ts"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(repo.join("src/client.ts"), "const a=1\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    // The person's own uncommitted edit, before the session.
    std::fs::write(repo.join("src/retry.ts"), "one\ntwo (mine)\nthree\n").unwrap();
    std::fs::write(repo.join("notes.md"), "untracked\n").unwrap();
    std::fs::write(repo.join(".env"), "TOKEN=x\n").unwrap();
    (base, repo)
}

/// Everything git keeps about the repository that a snapshot could touch.
fn state(repo: &Path) -> (String, String, Vec<u8>, usize) {
    let refs = git(repo, &["for-each-ref"]);
    let status = git(repo, &["status", "--porcelain=v2", "--untracked-files=all"]);
    let index = std::fs::read(repo.join(".git/index")).unwrap();
    let objects = walk(&repo.join(".git/objects"));
    (refs, status, index, objects)
}

fn walk(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| {
                    if e.path().is_dir() {
                        walk(&e.path())
                    } else {
                        1
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

fn start(store: &Store, id: &str, repo: &Path) {
    let r = store.repo_of(repo).expect("a repository");
    store
        .set_meta(
            id,
            &Meta {
                cwd: repo.display().to_string(),
                root: Some(r.root.display().to_string()),
                started: 1,
                ..Meta::default()
            },
        )
        .unwrap();
    store.snapshot(id, &r, Duration::from_secs(30)).unwrap();
}

#[test]
fn snapshot_leaves_the_repo_untouched() {
    let (base, repo) = repo("untouched");
    let store = Store::new(&base.join("support"));
    let before = state(&repo);
    start(&store, "sess-1", &repo);
    // Listing changes takes a second snapshot; that must not write either.
    std::fs::write(repo.join("src/client.ts"), "const a = 1;\n").unwrap();
    store.changes("sess-1").unwrap();
    std::fs::write(repo.join("src/client.ts"), "const a=1\n").unwrap();
    assert_eq!(state(&repo), before);
}

#[test]
fn snapshot_includes_untracked_not_ignored() {
    let (base, repo) = repo("untracked");
    let store = Store::new(&base.join("support"));
    start(&store, "sess-1", &repo);
    let notes = store.base_text("sess-1", &repo.join("notes.md")).unwrap();
    assert_eq!(notes.text.as_deref(), Some("untracked\n"));
    let env = store.base_text("sess-1", &repo.join(".env")).unwrap();
    assert!(!env.exists, "an ignored file is not in the snapshot");
}

#[test]
fn a_git_session_lists_edit_tool_and_formatter_changes_and_keeps_your_own() {
    let (base, repo) = repo("git-session");
    let store = Store::new(&base.join("support"));
    start(&store, "sess-1", &repo);
    // Claude's Edit, and a formatter run through Bash.
    std::fs::write(repo.join("src/retry.ts"), "one\ntwo (mine)\nthree\nfour\n").unwrap();
    std::fs::write(repo.join("src/client.ts"), "const a = 1;\n").unwrap();
    let changes = store.changes("sess-1").unwrap();
    assert!(changes.tree);
    let paths: Vec<_> = changes.files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(
        paths,
        [
            repo.join("src/client.ts").display().to_string(),
            repo.join("src/retry.ts").display().to_string()
        ]
    );
    let retry = &changes.files[1];
    assert_eq!(
        (retry.added, retry.removed),
        (Some(1), Some(0)),
        "your own edit is not Claude's"
    );
    let text = store
        .base_text("sess-1", &repo.join("src/retry.ts"))
        .unwrap();
    assert_eq!(text.text.as_deref(), Some("one\ntwo (mine)\nthree\n"));
}

#[test]
fn a_first_touch_copy_wins_over_the_snapshot() {
    let (base, repo) = repo("copy-wins");
    let support = base.join("support");
    let store = Store::new(&support);
    let file = repo.join("src/retry.ts");
    // The hook copies before an edit that lands before the snapshot is ready.
    first_touch(&store::store(&support), "sess-1", &file, SIZE_CAP).unwrap();
    std::fs::write(&file, "edited before the snapshot\n").unwrap();
    start(&store, "sess-1", &repo);
    let text = store.base_text("sess-1", &file).unwrap();
    assert_eq!(text.text.as_deref(), Some("one\ntwo (mine)\nthree\n"));
    let changes = store.changes("sess-1").unwrap();
    assert!(
        changes
            .files
            .iter()
            .any(|f| f.path == file.display().to_string() && f.removed == Some(3))
    );
}

#[test]
fn a_session_outside_git_lists_edit_tool_changes_only() {
    let base = folder("notes");
    let notes = base.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(notes.join("todo.md"), "a\nb\n").unwrap();
    std::fs::write(notes.join("log.txt"), "x\n").unwrap();
    let support = base.join("support");
    let store = Store::new(&support);
    assert!(store.repo_of(&notes).is_none());
    store
        .set_meta(
            "sess-2",
            &Meta {
                cwd: notes.display().to_string(),
                started: 1,
                ..Meta::default()
            },
        )
        .unwrap();
    first_touch(
        &store::store(&support),
        "sess-2",
        &notes.join("todo.md"),
        SIZE_CAP,
    )
    .unwrap();
    std::fs::write(notes.join("todo.md"), "a\nb\nc\n").unwrap();
    // `sed -i` through Bash: no hook copy, so it does not show.
    std::fs::write(notes.join("log.txt"), "y\n").unwrap();
    let changes = store.changes("sess-2").unwrap();
    assert!(!changes.tree);
    assert_eq!(changes.files.len(), 1);
    assert_eq!(
        changes.files[0].path,
        notes.join("todo.md").display().to_string()
    );
    assert_eq!(
        (changes.files[0].added, changes.files[0].removed),
        (Some(1), Some(0))
    );
}

#[test]
fn old_baselines_go() {
    let base = folder("old");
    let store = Store::new(&base.join("support"));
    let day = 24 * 3600;
    store
        .set_meta(
            "old-1",
            &Meta {
                started: 0,
                ended: Some(1000),
                ..Meta::default()
            },
        )
        .unwrap();
    store
        .set_meta(
            "new-1",
            &Meta {
                started: 0,
                ended: Some(1000 + 7 * day),
                ..Meta::default()
            },
        )
        .unwrap();
    store
        .set_meta(
            "run-1",
            &Meta {
                started: 0,
                ..Meta::default()
            },
        )
        .unwrap();
    let gone = store.prune(Duration::from_secs(7 * day), u64::MAX, 1000 + 7 * day + 10);
    assert_eq!(gone, ["old-1"]);
    assert!(store.meta("new-1").is_some() && store.meta("run-1").is_some());
}

#[test]
fn the_cap_spares_running_sessions() {
    let base = folder("cap");
    let store = Store::new(&base.join("support"));
    let fill = |id: &str, meta: Meta, bytes: usize| {
        store.set_meta(id, &meta).unwrap();
        std::fs::write(store.dir.join(id).join("blob"), vec![0u8; bytes]).unwrap();
    };
    fill(
        "a-1",
        Meta {
            ended: Some(10),
            ..Meta::default()
        },
        300,
    );
    fill(
        "b-1",
        Meta {
            ended: Some(20),
            ..Meta::default()
        },
        300,
    );
    fill("run-1", Meta::default(), 300);
    // A resumed session reads its baseline from an ended one, which then stays too.
    fill(
        "c-1",
        Meta {
            ended: Some(5),
            ..Meta::default()
        },
        300,
    );
    fill(
        "run-2",
        Meta {
            base: Some("c-1".into()),
            ..Meta::default()
        },
        10,
    );
    let gone = store.prune(Duration::from_secs(u64::MAX / 2), 700, 100);
    assert_eq!(gone, ["a-1", "b-1"]);
    for id in ["run-1", "c-1", "run-2"] {
        assert!(store.meta(id).is_some(), "{id}");
    }
}

#[test]
fn the_snapshot_keeps_secrets_and_big_files_out_of_the_store() {
    let (base, repo) = repo("excluded");
    std::fs::write(repo.join(".env.local"), "TOKEN=x\n").unwrap();
    std::fs::write(repo.join("server.pem"), "key\n").unwrap();
    std::fs::write(repo.join("-odd [name]*.txt"), "ok\n").unwrap();
    std::fs::write(repo.join("video.bin"), vec![0u8; SIZE_CAP as usize + 1]).unwrap();
    let store = Store::new(&base.join("support"));
    start(&store, "sess-1", &repo);
    for gone in [".env.local", "server.pem", "video.bin"] {
        let t = store.base_text("sess-1", &repo.join(gone)).unwrap();
        assert!(!t.exists, "{gone} is not in the snapshot");
    }
    for kept in ["notes.md", "-odd [name]*.txt"] {
        assert!(
            store.base_text("sess-1", &repo.join(kept)).unwrap().exists,
            "{kept}"
        );
    }
    // The second snapshot, taken by `changes`, leaves them out too.
    let changes = store.changes("sess-1").unwrap();
    assert!(changes.files.is_empty(), "{:?}", changes.files);
    // Ready names the root the snapshot covers.
    let ready = std::fs::read_to_string(store.dir.join("sess-1").join(store::READY)).unwrap();
    assert_eq!(ready.lines().nth(1), Some(repo.to_str().unwrap()));
}

#[test]
fn a_session_that_never_ended_is_pruned_when_stale() {
    let base = folder("stale");
    let store = Store::new(&base.join("support"));
    let day = 24 * 3600;
    for id in ["stale-1", "live-1"] {
        store.set_meta(id, &Meta::default()).unwrap();
        std::fs::create_dir_all(store.dir.join(id).join("files")).unwrap();
    }
    let old = std::time::SystemTime::now() - Duration::from_secs(30 * day);
    let touch = |p: PathBuf| std::fs::File::open(p).unwrap().set_modified(old).unwrap();
    let dir = store.dir.join("stale-1");
    touch(dir.join("meta.json"));
    touch(dir.join("files"));
    touch(dir);
    let gone = store.prune(
        Duration::from_secs(7 * day),
        u64::MAX,
        terminescu_plugin_claude::baseline::now(),
    );
    assert_eq!(gone, ["stale-1"]);
    assert!(store.meta("live-1").is_some());
}
