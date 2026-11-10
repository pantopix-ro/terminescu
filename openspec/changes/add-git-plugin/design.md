# Design: add-git-plugin

## Context

See proposal.md for why. The plugin builds on add-plugin-architecture: it observes cwd and
command events, pushes decorations, sets status facts, provides live variables, stores
per-row data, and registers a panel section, a pane view and actions. The ptyd already
reports each pane's cwd through OSC 7 and command ends through `OSC 633`, so the plugin
needs no polling to know when to look. The core already extracts GitHub pull request links
(`extract.rs`).

## Goals / Non-Goals

**Goals:** git facts that are never stale by more than a second after a command, never slow a
terminal and never touch the repository unless the person asks; worktree rows in one step.

**Non-Goals:** a git client (commit, stage, push, branch delete); forges other than GitHub;
file watching of whole working trees.

## Decisions

### D1. A version-control provider strategy, `git` CLI first
```
trait Vcs: Send + Sync {
    fn facts(&self, dir: &Path) -> Result<Option<RepoFacts>, VcsError>;   // None: not a repo
    fn changes(&self, root: &Path) -> Result<Vec<Change>, VcsError>;
    fn base_text(&self, root: &Path, file: &Path, base: DiffBase) -> Result<Option<Vec<u8>>, VcsError>;
    fn worktree_add(&self, root: &Path, branch: &str, base: &str, at: &Path) -> Result<(), VcsError>;
    fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>, VcsError>;
    fn worktree_remove(&self, root: &Path, at: &Path, force: bool) -> Result<(), VcsError>;
}
```
`GitCli` runs the person's `git` with `--no-optional-locks`, `GIT_OPTIONAL_LOCKS=0`,
`LC_ALL=C` and `-c core.quotepath=off`, and parses `status --porcelain=v2 --branch -z`,
`diff --numstat -z` and `worktree list --porcelain`. Rejected: `gix` or `git2`. They add a
large dependency and differ from the person's git on config, hooks and new index formats.

### D2. Decorators around the provider
`TimedVcs` kills a call after 1 second. `CachedVcs` keeps facts per repository root and reuses
them until `.git/HEAD`, `.git/index` or the worktree's `HEAD` change mtime, or 2 seconds pass.
`CoalescingVcs` folds requests for one root that arrive while a call runs into one follow-up.
The plugin builds `CoalescingVcs(CachedVcs(TimedVcs(GitCli)))`. Each layer has its own unit
tests over a fake inner provider.

### D3. Triggers, not polling
Facts refresh on cwd moved, command ended, tab selected and app launched, debounced 300 ms per
pane. The debounce is a host timer: `PluginCx::schedule(after, token)` sends the plugin
`CoreEvent::Timer` on its own worker, under its event budget, and setting a token again moves
it. A plugin keeps no thread of its own, so pause and budgets still hold. Pull request facts refresh on command ended in that tab and at most once a minute per
branch. Rejected: a file watcher on each working tree. FSEvents on large trees is costly and
the shell events already mark every moment that matters.

### D4. A forge strategy for GitHub, through `gh`
`trait Forge { available; pr_for(root, branch); open_prs(root); checkout(dir, number) }`,
implemented by `GhCli` with `gh pr list --state open --head <branch> --json ...` (so a merged
pull request of the branch never shows) and `gh pr list --json`. A pull request row starts as a
detached worktree; `gh pr checkout` then makes its branch inside it, forks included. Lookups run
on a thread of their own, so a 3-second `gh` never holds the plugin's worker; short timers pick
the answer up. The GitHub part is on when `gh auth status` succeeds and the setting allows it.
`gh` keeps its own token; Terminescu never reads it. Rejected: the GitHub REST API with a token
in the Keychain. It breaks the no-token rule's spirit and duplicates `gh`.

### D5. Worktree paths
Default `<parent of repo root>/<repo name>-<branch slug>`; with a folder set,
`<folder>/<repo name>/<branch slug>`. The slug replaces `/` and anything outside
`[A-Za-z0-9._-]` with `-`. An existing path gets `-2`, `-3`. The row stores
`{ worktree: path, repo: main root }` in `plugin_side_tab_data`, so the close question only
shows for rows the plugin made or opened as worktree rows.

### D6. The diff tab is a decorated text tab
The pane view `git-diff` wraps the text editor with `@codemirror/merge` (MergeView side by
side, `unifiedMergeView` unified). The working copy side is the same document model as a text
tab, so saving, encodings and line endings follow `text-editor` unchanged. The base text comes
from `pluginCall("git", "baseText")`. Rejected: a separate diff renderer. Editing in place is
the point.

### D7. Chips, facts and variables
Decorations: tab chip `branch` (text, tone neutral, red on conflict, tip with upstream and
counts), tab chip `pr` (number, tone by checks). Status facts: `git.pushed`, `git.conflict`,
`git.checksFailed`. Variables: `branch`, `repo`, `pr`. A folder row shows its shown tab's
chips through the row decorator in `packages/plugin-git`.

## Risks / Trades-offs

- [The person's git is old or unusual] → Require 2.30 (porcelain v2 and `worktree list
  --porcelain`); below that the plugin stops after one log line and Settings says why.
- [`gh` is slow or rate-limited] → One call per branch per minute at most, 3-second timeout,
  the last answer kept.
- [Removing the wrong worktree] → Only rows the plugin marked can offer removal; `git worktree
  remove` without `--force` first; with changes, a second question naming the count.
- [Hooks in the person's repository run on `worktree add`] → They are the person's hooks and
  run as they would from a shell; the dialog says the command it runs.

## Migration Plan

No migration. The plugin ships behind cargo feature `plugin-git` and its switch in Settings ›
Plugins.
