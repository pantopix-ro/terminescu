//! Worktrees: where they go, and how they come and go (add-git-plugin 1.3).

mod common;

use common::*;
use terminescu_plugin_git::worktree::path_for;
use terminescu_plugin_git::{Head, Vcs, VcsError};

#[test]
fn worktree_goes_next_to_the_repo() {
    let repo = repo("shop-api");
    let at = path_for(&repo, "shop-api", "PROJ-490-refunds", None);
    assert_eq!(at, repo.parent().unwrap().join("shop-api-PROJ-490-refunds"));
    let cli = git_cli();
    cli.worktree_add(&repo, "PROJ-490-refunds", "main", &at)
        .unwrap();
    let f = cli.facts(&at).unwrap().unwrap();
    assert_eq!(f.root, at);
    assert_eq!(f.head, Head::Branch("PROJ-490-refunds".into()));
    assert_eq!(f.main_root(), repo);
    assert_eq!(f.repo_name(), "shop-api");
    let list = cli.worktrees(&repo).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list[0].main && list[0].path == repo);
    assert_eq!(list[1].branch.as_deref(), Some("PROJ-490-refunds"));
}

#[test]
fn worktree_goes_under_the_folder() {
    let repo = repo("shop-api");
    let folder = folder("worktrees");
    let at = path_for(&repo, "shop-api", "PROJ-490-refunds", Some(&folder));
    assert_eq!(at, folder.join("shop-api/PROJ-490-refunds"));
    git_cli()
        .worktree_add(&repo, "PROJ-490-refunds", "main", &at)
        .unwrap();
    assert!(at.join("a.txt").is_file());
}

#[test]
fn an_existing_path_gets_a_suffix() {
    let repo = repo("shop-api");
    let taken = repo.parent().unwrap().join("shop-api-PROJ-490");
    std::fs::create_dir_all(&taken).unwrap();
    let second = path_for(&repo, "shop-api", "PROJ-490", None);
    assert_eq!(second, repo.parent().unwrap().join("shop-api-PROJ-490-2"));
    std::fs::create_dir_all(&second).unwrap();
    assert!(path_for(&repo, "shop-api", "PROJ-490", None).ends_with("shop-api-PROJ-490-3"));
    // git is never asked to fill a folder that exists.
    assert!(matches!(
        git_cli().worktree_add(&repo, "PROJ-490", "main", &taken),
        Err(VcsError::Exists(_))
    ));
}

#[test]
fn remove_with_changes_needs_force() {
    let repo = repo("shop-api");
    let at = path_for(&repo, "shop-api", "PROJ-491", None);
    let cli = git_cli();
    cli.worktree_add(&repo, "PROJ-491", "main", &at).unwrap();
    write(&at, "a.txt", "changed\n");
    assert_eq!(
        cli.worktree_remove(&repo, &at, false),
        Err(VcsError::HasChanges)
    );
    assert!(at.exists());
    cli.worktree_remove(&repo, &at, true).unwrap();
    assert!(!at.exists());
    // The branch stays.
    assert_eq!(
        git(&repo, &["branch", "--list", "PROJ-491"]).trim(),
        "PROJ-491"
    );
}

#[test]
fn a_clean_worktree_goes_without_force_and_a_branch_checks_out() {
    let repo = repo("shop-api");
    git(&repo, &["branch", "pr-918"]);
    let at = path_for(&repo, "shop-api", "pr-918", None);
    let cli = git_cli();
    cli.worktree_checkout(&repo, "pr-918", &at).unwrap();
    assert_eq!(
        cli.facts(&at).unwrap().unwrap().head,
        Head::Branch("pr-918".into())
    );
    cli.worktree_remove(&repo, &at, false).unwrap();
    assert!(!at.exists());
}

#[test]
fn a_name_that_reads_as_an_option_is_refused() {
    let repo = repo("shop-api");
    let at = repo.parent().unwrap().join("x");
    let cli = git_cli();
    assert!(cli.worktree_add(&repo, "--orphan", "main", &at).is_err());
    assert!(cli.worktree_add(&repo, "ok", "--detach", &at).is_err());
    assert!(cli.worktree_checkout(&repo, "-b", &at).is_err());
    assert!(!at.exists());
}
