//! `GitCli` against real temporary repositories (add-git-plugin 1.1).

mod common;

use common::*;
use std::path::Path;
use terminescu_plugin_git::{DiffBase, Head, Op, Vcs, VcsError};

#[test]
fn a_folder_outside_a_repository_has_no_facts() {
    let dir = folder("plain");
    assert_eq!(git_cli().facts(&dir).unwrap(), None);
}

#[test]
fn a_clean_repository() {
    let dir = repo("shop-api");
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.root, dir);
    assert_eq!(f.head, Head::Branch("main".into()));
    assert_eq!((f.changed, f.conflicted, f.ahead, f.behind), (0, 0, 0, 0));
    assert_eq!(f.op, None);
    assert_eq!(f.repo_name(), "shop-api");
    assert_eq!(f.main_root(), dir);
    // A subfolder answers for the same repository.
    std::fs::create_dir_all(dir.join("src")).unwrap();
    assert_eq!(
        git_cli().facts(&dir.join("src")).unwrap().unwrap().root,
        dir
    );
}

#[test]
fn a_dirty_tree_counts_changed_staged_and_untracked_files() {
    let dir = repo("dirty");
    write(&dir, "a.txt", "one\n2\nthree\n");
    write(&dir, "b.txt", "new\n");
    git(&dir, &["add", "b.txt"]);
    write(&dir, "notes/todo.md", "x\ny\n");
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.changed, 3);
    let c = git_cli().changes(&dir).unwrap();
    let paths: Vec<_> = c.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["a.txt", "b.txt", "notes/todo.md"]);
    assert!(c[0].unstaged && !c[0].staged);
    assert_eq!((c[0].added, c[0].removed), (Some(2), Some(1)));
    assert!(c[1].staged);
    assert_eq!((c[1].added, c[1].removed), (Some(1), Some(0)));
    assert!(c[2].untracked);
    assert_eq!((c[2].added, c[2].removed), (Some(2), Some(0)));
}

#[test]
fn ahead_and_behind_the_upstream() {
    let dir = repo("ab");
    git(&dir, &["switch", "-qc", "feature/PROJ-482-retry"]);
    git(&dir, &["branch", "-q", "--set-upstream-to=main"]);
    for i in 0..2 {
        write(&dir, "a.txt", &format!("{i}\n"));
        git(&dir, &["commit", "-qam", "work"]);
    }
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "c.txt", "c\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "main moves"]);
    git(&dir, &["switch", "-q", "feature/PROJ-482-retry"]);
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.head, Head::Branch("feature/PROJ-482-retry".into()));
    assert_eq!(f.upstream.as_deref(), Some("main"));
    assert_eq!((f.ahead, f.behind), (2, 1));
    assert!(!f.pushed());
}

#[test]
fn a_detached_head_shows_its_commit() {
    let dir = repo("detached");
    let sha = git(&dir, &["rev-parse", "--short=7", "HEAD"]);
    git(&dir, &["switch", "-q", "--detach"]);
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.head, Head::Detached(sha.trim().into()));
    assert_eq!(f.head.branch(), None);
}

#[test]
fn a_stopped_rebase_shows_its_conflict() {
    let dir = repo("rebase");
    git(&dir, &["switch", "-qc", "topic"]);
    write(&dir, "a.txt", "topic\n");
    git(&dir, &["commit", "-qam", "topic"]);
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "a.txt", "main\n");
    git(&dir, &["commit", "-qam", "main"]);
    git(&dir, &["switch", "-q", "topic"]);
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(["rebase", "main"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.op, Some(Op::Rebase));
    assert_eq!(f.conflicted, 1);
    assert!(git_cli().changes(&dir).unwrap()[0].conflicted);
}

#[test]
fn a_rename_names_both_paths() {
    let dir = repo("rename");
    git(&dir, &["mv", "a.txt", "renamed.txt"]);
    let c = git_cli().changes(&dir).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].path, "renamed.txt");
    assert_eq!(c[0].from.as_deref(), Some("a.txt"));
    assert!(c[0].staged);
}

#[test]
fn an_unborn_branch_has_facts_and_changes() {
    let dir = folder("unborn");
    git(&dir, &["init", "-q"]);
    write(&dir, "a.txt", "x\n");
    let f = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(f.head, Head::Unborn("main".into()));
    assert_eq!(f.changed, 1);
    assert_eq!(git_cli().changes(&dir).unwrap()[0].added, Some(1));
}

#[test]
fn base_text_reads_head_and_the_staged_version() {
    let dir = repo("base");
    write(&dir, "a.txt", "staged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "a.txt", "working\n");
    let cli = git_cli();
    let head = cli
        .base_text(&dir, Path::new("a.txt"), DiffBase::Head)
        .unwrap();
    assert_eq!(head.as_deref(), Some(b"one\ntwo\n".as_slice()));
    let index = cli
        .base_text(&dir, Path::new("a.txt"), DiffBase::Index)
        .unwrap();
    assert_eq!(index.as_deref(), Some(b"staged\n".as_slice()));
    assert_eq!(
        cli.base_text(&dir, Path::new("new.txt"), DiffBase::Head)
            .unwrap(),
        None
    );
    assert!(
        cli.base_text(&dir, Path::new("../a.txt"), DiffBase::Head)
            .is_err()
    );
}

#[test]
fn status_never_leaves_an_index_lock() {
    let dir = repo("locks");
    write(&dir, "a.txt", "changed\n");
    for _ in 0..3 {
        git_cli().facts(&dir).unwrap();
        git_cli().changes(&dir).unwrap();
    }
    assert!(!dir.join(".git/index.lock").exists());
}

#[test]
fn a_missing_git_is_named() {
    let cli = terminescu_plugin_git::GitCli::new("/nonexistent/git");
    assert_eq!(cli.facts(Path::new("/tmp")), Err(VcsError::NoGit));
}

#[test]
fn a_slow_git_is_killed_at_its_limit() {
    let dir = folder("slow");
    let script = dir.join("slow-git");
    std::fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap();
    let cli = terminescu_plugin_git::GitCli::new(&script)
        .with_limit(std::time::Duration::from_millis(200));
    let start = std::time::Instant::now();
    assert_eq!(cli.facts(&dir), Err(VcsError::Timeout));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn the_installed_git_is_new_enough() {
    let v = git_cli().version().unwrap();
    assert!(v.starts_with('2'), "{v}");
}

#[test]
fn a_background_process_holding_the_pipes_does_not_hold_the_call() {
    let dir = folder("lingering");
    let script = dir.join("git");
    // A hook that leaves a process behind keeps git's stdout open after git exits.
    std::fs::write(&script, "#!/bin/sh\nsleep 5 &\necho 'git version 2.44.0'\n").unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap();
    // macOS scans a freshly written script on its first run, which on a loaded CI machine can
    // take over the default 1 s; the limit stays below the 5 s the background sleep holds.
    let cli =
        terminescu_plugin_git::GitCli::new(&script).with_limit(std::time::Duration::from_secs(4));
    assert_eq!(cli.version().as_deref(), Ok("2.44.0"));
}

#[test]
fn a_folder_reached_through_a_link_has_the_resolved_common_dir() {
    let dir = repo("linked");
    let link = folder("links").join("shop-api");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let facts = git_cli().facts(&link).unwrap().unwrap();
    assert_eq!(facts.root, dir);
    assert_eq!(facts.common_dir, dir.join(".git"));
    assert_eq!(facts.main_root(), dir);
}

#[test]
fn an_untracked_link_is_counted_as_one_line_and_never_followed() {
    let dir = repo("links-untracked");
    std::os::unix::fs::symlink("/dev/zero", dir.join("zero")).unwrap();
    let changes = git_cli().changes(&dir).unwrap();
    let zero = changes.iter().find(|c| c.path == "zero").unwrap();
    assert!(zero.untracked);
    assert_eq!(zero.added, None);
}

#[test]
fn an_untracked_folder_counts_once_in_facts_and_lists_each_file_in_changes() {
    let dir = repo("untracked-folder");
    write(&dir, "build/a.txt", "a\n");
    write(&dir, "build/b.txt", "b\n");
    let facts = git_cli().facts(&dir).unwrap().unwrap();
    assert_eq!(facts.changed, 1);
    assert_eq!(git_cli().changes(&dir).unwrap().len(), 2);
}

#[test]
fn untracked_files_past_the_count_budget_show_no_counts() {
    let dir = repo("many-untracked");
    let n = terminescu_plugin_git::cli::COUNT_FILES_UP_TO + 5;
    for i in 0..n {
        write(&dir, &format!("out/f{i:03}.txt"), "x\n");
    }
    let changes = git_cli().changes(&dir).unwrap();
    assert_eq!(changes.len(), n);
    let counted = changes.iter().filter(|c| c.added.is_some()).count();
    assert_eq!(counted, terminescu_plugin_git::cli::COUNT_FILES_UP_TO);
}

#[test]
fn a_base_text_is_sized_before_it_is_read() {
    let dir = repo("sized-base");
    let text = git_cli()
        .base_text(&dir, Path::new("a.txt"), DiffBase::Head)
        .unwrap();
    assert_eq!(text.as_deref(), Some(&b"one\ntwo\n"[..]));
}
