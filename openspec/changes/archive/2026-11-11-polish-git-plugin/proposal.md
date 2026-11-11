## Why

The review of the git plugin after harden-plugin-host found twelve rough edges. The pull
request tick reads git every minute for nothing. Chips flicker on every read. A diff tab jumps to
the top whenever HEAD moves. Bare repositories put worktrees in the wrong place. A worktree row
that closes on a shell exit leaves its worktree behind without a word. Renamed and deleted files
open the wrong diff.

## What Changes

- The pull request tick works from the facts it has, and stops on a pane with no branch.
- A tab's decorations change only where they differ; nothing is cleared and set again.
- The `facts` event names the repository root, so a diff tab reads its base only for its own
  repository.
- Diff colours live in an editor theme; the global rules go.
- A diff tab keeps its scroll place: a new base updates the open editor, a layout switch puts the
  scroll back, and the side-by-side view takes new text without being built again.
- `gh auth status` asks about github.com only.
- Bare repositories, `repo.git` and the `.bare` layout, name the repository and the main folder
  right. A row whose worktree git cannot read closes without blocking.
- The worktree folder from Settings is normalised: trailing slashes and `.` parts go.
- New Row from Worktree shows the folder the worktree goes to and the base branch.
- A worktree row that closed without asking (its shell exited) shows a notice: "Worktree kept at
  X. Remove it?"
- A renamed file diffs against its old path at HEAD.
- A deleted file opens read-only with a "Deleted" note, in the git and the Claude diff tabs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `git-integration`: the requirements below are added to the change add-git-plugin made.

## Impact

`crates/plugin-git` (plugin.rs, vcs.rs, forge.rs), `packages/plugin-git`, `packages/ui`
(DiffPane, diff-merge), `packages/plugin-claude` (ChangesSection), the git e2e spec and
`docs/plugins.md`. No migration. No new dependency.

## Unresolved questions

None. Defaults: the orphan notice shows in the window that held the row; Remove there never
forces, and a worktree with changes asks again.
