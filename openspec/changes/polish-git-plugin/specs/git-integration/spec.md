## ADDED Requirements

### Requirement: Quiet refresh

The pull request tick SHALL read no git. A tab's chips and facts SHALL change only where their
value changed.

#### Scenario: Same facts twice

- **WHEN** a pane's facts are read twice with the same result
- **THEN** no decoration is cleared or set the second time

#### Scenario: No branch

- **WHEN** the lead pane's facts hold no branch
- **THEN** its pull request tick stops

### Requirement: Diff tab keeps its place

A diff tab SHALL keep its scroll place when its base changes and when its layout switches. A
`facts` event SHALL name its repository root, and a diff tab SHALL read its base only for its own
root.

#### Scenario: Commit while scrolled

- **WHEN** the person scrolls a diff tab down and commits in another tab
- **THEN** the diff tab shows the new base at the same place

### Requirement: Bare repositories

The plugin SHALL name a bare repository and its main folder from the shared git folder, so
worktrees go beside it. A row whose worktree git cannot read SHALL close without blocking.

#### Scenario: repo.git

- **WHEN** a worktree of `shop.git` makes a new worktree row
- **THEN** the worktree goes to `shop-<branch>` beside `shop.git`

### Requirement: Orphan worktree notice

A worktree row that closes without asking SHALL show a notice naming the kept worktree, with
Remove and Keep. Remove SHALL never force without asking again.

#### Scenario: Shell exits in a worktree row

- **WHEN** the only shell of a worktree row exits
- **THEN** a notice reads "Worktree kept at <path>. Remove it?"

### Requirement: Renamed and deleted files

A renamed file SHALL diff against its old path at HEAD. A deleted file SHALL open read-only with
a "Deleted" note, in the git and the Claude diff tabs.

#### Scenario: Rename

- **WHEN** the person opens a file renamed from `a.ts` to `b.ts`
- **THEN** the diff compares `b.ts` with `a.ts` at HEAD

### Requirement: Worktree dialog names its target

New Row from Worktree SHALL show the folder the worktree goes to and the base branch.

#### Scenario: Default folder

- **WHEN** the person opens the dialog in `shop` on `main`
- **THEN** the detail reads the folder `shop-<branch>` beside `shop`, from `main`
