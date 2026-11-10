## Purpose

The git plugin shows which repository, branch and worktree each row and tab works in, what
changed there, and the GitHub pull request behind it, and makes a worktree row in one step.

## ADDED Requirements

### Requirement: Repository facts per pane
For each terminal pane whose cwd lies in a git repository, the plugin SHALL know the repository
root, the branch or the detached commit, the number of changed files, the commits ahead of and
behind the upstream, and whether a merge, rebase or cherry-pick is in progress. The facts SHALL
refresh within 1 second after the cwd moves or a command ends.

#### Scenario: cd into a repository
- **WHEN** the person runs `cd ~/code/shop-api` in a shell tab and the repository is on
  `feature/PROJ-482-retry` with 3 changed files
- **THEN** within a second the tab knows branch `feature/PROJ-482-retry` and 3 changed files

### Requirement: Git never slows a terminal
A git call SHALL never block typing. One that takes longer than 1 second SHALL be dropped and
retried on the next trigger. The plugin SHALL NOT take git's index lock.

#### Scenario: A huge repository
- **WHEN** `git status` takes 4 seconds in the pane's repository
- **THEN** typing echoes at once, the chip keeps its last facts, and no `index.lock` appears

### Requirement: Branch chips on rows and tabs
A tab whose shown pane is in a repository SHALL carry a chip with its branch name, a dot when
files changed, and `↑n` and `↓n` for commits ahead and behind. A one-tab row SHALL carry its
tab's chip; a folder SHALL carry its shown tab's chip. While a merge or rebase stops on a
conflict, the chip SHALL turn red and its tip SHALL say which. A long branch name SHALL shorten
in the middle.

#### Scenario: Ahead of upstream
- **WHEN** the branch is 2 commits ahead of its upstream with a clean tree
- **THEN** the chip reads `feature/PROJ-482-retry ↑2` with no dot

#### Scenario: A rebase stops
- **WHEN** `git rebase develop` stops on a conflict in the tab's repository
- **THEN** the chip turns red and its tip reads "Rebase stopped on a conflict"

### Requirement: Changes in the context panel
The context panel SHALL show a Changes section for the active tab's repository, listing each
changed file with lines added and removed and marked staged, unstaged or untracked, sorted by
path. Selecting a file SHALL open its diff tab. A repository with more than 500 changed files
SHALL show the first 500 and the total.

#### Scenario: Open a change
- **WHEN** the Changes section lists `src/retry.ts +12 −3` and the person selects it
- **THEN** a diff tab for `src/retry.ts` opens beside the focused pane

### Requirement: Diff tab
A diff tab SHALL show a file against HEAD, or against the staged version when it has staged
changes, side by side or unified, as Settings › Git sets and a button in the tab toggles. The
working copy side SHALL be editable and SHALL save like a text tab; the other side SHALL be
read-only. A binary file SHALL show "Binary file changed" with both sizes.

#### Scenario: Edit in the diff
- **WHEN** the person fixes a line on the working copy side and presses `⌘S`
- **THEN** the file saves byte-exact and the diff updates

### Requirement: Worktree rows
"New row from worktree…" in the sidebar's + menu and the palette SHALL ask for a branch name
and a base (the current branch by default), create the branch and a worktree for it, and open a
row named after the branch, below the selected row, whose first tab's login shell starts in the
worktree. "Open worktrees as rows" SHALL open a row for each of a repository's worktrees that no
row shows yet.

#### Scenario: New worktree row
- **WHEN** the person picks "New row from worktree…" in a row on `develop` of `shop-api` and
  types `PROJ-490-refunds`
- **THEN** branch `PROJ-490-refunds` and worktree `../shop-api-PROJ-490-refunds` exist, a row
  named `PROJ-490-refunds` opens below the selected one, and its shell's cwd is the worktree

### Requirement: Worktree location
Settings › Git SHALL choose where worktrees go: next to the repository as `<repo>-<branch>`
(default) or under a chosen folder as `<folder>/<repo>/<branch>`.

#### Scenario: Worktrees under a chosen folder
- **WHEN** Settings › Git sets the worktree folder to `~/worktrees` and the person makes
  `PROJ-490-refunds` from `shop-api`
- **THEN** the worktree is `~/worktrees/shop-api/PROJ-490-refunds`

### Requirement: Closing a worktree row
Closing a row the plugin made or opened as a worktree row SHALL offer Keep worktree (default)
or Remove worktree. Removing one with changes SHALL ask again and name the count. The plugin
SHALL NEVER delete a branch.

#### Scenario: Close a worktree row with changes
- **WHEN** the person closes that row, picks Remove worktree, and the worktree has 2 changed files
- **THEN** a second question says 2 changed files would be lost, and Cancel keeps the worktree
  and the branch

### Requirement: GitHub pull requests through gh
When `gh` is installed and logged in and the GitHub part is on, the branch chip SHALL show the
branch's open pull request number and its checks as passing, failing or running, refreshed at
most once a minute and when a command ends in that tab. "Open pull request" SHALL open it in the
browser. "Check out pull request as a row…" SHALL list open pull requests and open the picked
one as a worktree row. Without `gh`, these items SHALL not show. Terminescu SHALL NOT read or
store a GitHub token.

#### Scenario: Checks fail
- **WHEN** pull request 918 for the branch has a failing check
- **THEN** the chip shows `#918` in red and its tip names the failing check

#### Scenario: No gh
- **WHEN** `gh` is not on the PATH
- **THEN** the chips show branches only and no pull request item shows in menus or the palette

### Requirement: Live git variables
Prompts SHALL be able to use `{{branch}}`, `{{repo}}` and `{{pr}}`, filled from the focused
pane's repository when the prompt is inserted. They SHALL be read-only, SHALL not be saved to
the context, and a saved variable of the same name SHALL win.

#### Scenario: Insert a prompt with the branch
- **WHEN** the person inserts "Review {{branch}} against develop" in a tab on `PROJ-482-retry`
- **THEN** the terminal receives "Review PROJ-482-retry against develop" and no Enter

### Requirement: Git facts for statuses
Dynamic status rules SHALL be able to test "branch pushed", "has conflicts" and "pull request
checks failed" for a row's shown tab.

#### Scenario: A failing pull request status
- **WHEN** a rule sets status "CI red" when pull request checks failed, and the row's checks fail
- **THEN** the row shows "CI red" within a minute
