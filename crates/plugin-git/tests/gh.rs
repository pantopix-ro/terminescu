//! `GhCli` over a fake `gh` (add-git-plugin 2.2): an open pull request with a failing check,
//! no `gh`, and a `gh` that is logged out.

mod common;

use common::folder;
use std::path::{Path, PathBuf};
use terminescu_plugin_git::{Checks, Forge, ForgeError, GhCli};

const FAKE: &str = r#"#!/bin/sh
case "$1 $2" in
  "auth status")
    if [ "$FAKE_GH" = "out" ]; then
      echo "You are not logged into any GitHub hosts. To log in, run: gh auth login" >&2
      exit 1
    fi
    exit 0 ;;
  "pr list")
    if [ "$FAKE_GH" = "out" ]; then
      echo "To get started with GitHub CLI, please run:  gh auth login" >&2
      exit 4
    fi
    echo "$@" > "$FAKE_GH_ARGS"
    cat "$FAKE_GH_PRS" ;;
  "pr checkout")
    echo "$3" > checked-out ;;
  *) exit 1 ;;
esac
"#;

const PRS: &str = r#"[{"number": 918, "title": "Retry failed payments",
  "url": "https://github.com/acme/shop-api/pull/918", "headRefName": "feature/PROJ-482-retry",
  "state": "OPEN", "statusCheckRollup": [
    {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS"},
    {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE"}]}]"#;

fn fake(dir: &Path, mode: &str) -> (GhCli, PathBuf) {
    let script = dir.join("gh");
    std::fs::write(&script, FAKE).unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap();
    let prs = dir.join("prs.json");
    std::fs::write(&prs, PRS).unwrap();
    let args = dir.join("args");
    let gh = GhCli::new(&script)
        .with_env("FAKE_GH", mode)
        .with_env("FAKE_GH_PRS", &prs)
        .with_env("FAKE_GH_ARGS", &args);
    (gh, args)
}

#[test]
fn an_open_pull_request_with_a_failing_check() {
    let dir = folder("gh-open");
    let (gh, args) = fake(&dir, "in");
    assert_eq!(gh.available(), Ok(()));
    let pr = gh.pr_for(&dir, "feature/PROJ-482-retry").unwrap().unwrap();
    assert_eq!(pr.number, 918);
    assert_eq!(pr.checks, Checks::Failing(vec!["lint".into()]));
    let asked = std::fs::read_to_string(args).unwrap();
    assert!(asked.contains("--head feature/PROJ-482-retry"), "{asked}");
    assert_eq!(gh.open_prs(&dir).unwrap().len(), 1);
    // A branch that reads as an option is never passed on.
    assert_eq!(gh.pr_for(&dir, "--web"), Ok(None));
}

#[test]
fn a_checkout_runs_in_the_worktree() {
    let dir = folder("gh-checkout");
    let (gh, _) = fake(&dir, "in");
    let worktree = folder("gh-checkout-wt");
    gh.checkout(&worktree, 918).unwrap();
    let got = std::fs::read_to_string(worktree.join("checked-out")).unwrap();
    assert_eq!(got.trim(), "918");
}

#[test]
fn no_gh_and_logged_out_are_named() {
    let dir = folder("gh-missing");
    let missing = GhCli::new(dir.join("no-such-gh"));
    assert_eq!(missing.available(), Err(ForgeError::NoGh));
    let (gh, _) = fake(&dir, "out");
    assert_eq!(gh.available(), Err(ForgeError::LoggedOut));
    assert_eq!(gh.pr_for(&dir, "main"), Err(ForgeError::LoggedOut));
}
