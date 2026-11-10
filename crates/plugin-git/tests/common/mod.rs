//! Temporary repositories for the tests, with the person's git config left out.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use terminescu_plugin_git::GitCli;

static N: AtomicU32 = AtomicU32::new(0);

/// A fresh, empty folder.
pub fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!(
            "terminescu-git-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // macOS reaches the temp folder through /private; git reports that path.
    dir.canonicalize().unwrap()
}

pub fn git_cli() -> GitCli {
    GitCli::default()
        .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
        .with_env("GIT_CONFIG_NOSYSTEM", "1")
}

/// Runs git for the test's own setup, and fails the test on an error.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A repository on `main` with one commit of `a.txt`.
pub fn repo(name: &str) -> PathBuf {
    let dir = folder(name);
    git(&dir, &["init", "-q"]);
    write(&dir, "a.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    dir
}
