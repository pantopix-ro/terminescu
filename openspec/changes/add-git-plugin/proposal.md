# Proposal: add-git-plugin

## Why

People who run several Claude sessions give each one a branch, and often a worktree. Terminescu
shows a row per ticket but not which branch or worktree it works in, nor what changed there.
They switch to another tool to see a diff or to make a worktree. A git plugin closes that gap
on top of `add-plugin-architecture`, and the app still works without it.

## What Changes

- Repository facts per pane from its cwd: repository root, branch or detached commit, dirty
  file count, ahead and behind its upstream, and a merge, rebase or cherry-pick in progress.
  They refresh when the cwd moves or a command ends, and never block a terminal.
- A branch chip on each row and tab, with dirty and ahead/behind marks, and a conflict tone
  while a merge or rebase stops.
- A Changes section in the context panel: changed files with lines added and removed, staged
  and untracked marked. A click opens the diff tab.
- A diff tab: a text tab in side-by-side or unified diff mode against HEAD or the staged
  version, read-only on the left, editable on the right.
- Worktree rows: "New row from worktree…" makes a branch and a worktree, names the row after
  the branch and opens a login shell there. "Open worktrees as rows" lists a repository's
  worktrees. Closing a worktree row offers to remove the worktree; it never deletes a branch.
  Where worktrees go is a setting: next to the repository or under a chosen folder.
- GitHub through the `gh` CLI, when installed and logged in: the pull request and its checks on
  the branch chip, "Open pull request", and "Check out pull request as a row" into a new
  worktree. Terminescu never reads a GitHub token; `gh` keeps its own.
- Live variables `{{branch}}`, `{{repo}}` and `{{pr}}` for prompts, read-only, from the focused
  pane's repository.
- Git facts for dynamic statuses: "branch pushed", "has conflicts", "pull request checks failed".
- Settings › Git: worktree location, the default diff mode, and a switch for the GitHub part.

## Capabilities

### New Capabilities

- `git-integration`: repository facts, branch chips, changes and diffs, worktree rows, GitHub
  pull requests through `gh`, live git variables and git status facts.

### Modified Capabilities

None. The plugin uses the slots of `plugins` (add-plugin-architecture).

## Impact

- New crate `crates/plugin-git` (`terminescu-plugin-git`) and package `packages/plugin-git`,
  registered behind cargo feature `plugin-git`.
- New UI dependency `@codemirror/merge` 6.12.3 (MIT, released 2026-10-19), in
  `packages/plugin-git` only.
- Runs the person's `git` (2.30 or later) and, when present, `gh`. No git library is linked.
- Per-row worktree facts in `plugin_side_tab_data` (migration 0013 from
  add-plugin-architecture). No new migration.
- Depends on add-plugin-architecture. The pull request chip reuses the PR links that context
  extraction already finds.

Out of scope: committing, pushing, staging and branch deletion from the UI; other forges than
GitHub; jj or other version control systems (the provider strategy leaves room for them).
