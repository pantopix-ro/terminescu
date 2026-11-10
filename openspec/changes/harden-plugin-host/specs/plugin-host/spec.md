## ADDED Requirements

### Requirement: Every close can come back

A shell that exits and leaves its tab or row empty SHALL close it through the same path as a
close the person asks for, so ⇧⌘T brings it back. `⌘↵` in an edit pane SHALL close through the
app's close path.

#### Scenario: Exit closes a row

- **WHEN** the only shell of a row exits
- **THEN** the row closes, and ⇧⌘T brings it back with its context

### Requirement: A closed row's data reaches its plugin

The row-closed event SHALL carry the receiving plugin's own row data, as it stood before the
close.

#### Scenario: Worktree mark on exit

- **WHEN** a worktree row closes because its shell exited
- **THEN** the git plugin's close event holds the row's worktree mark

### Requirement: Reopen keeps its place

A reopened row SHALL show the tab it showed when it closed. A reopen that fails SHALL keep its
entry and show why.

#### Scenario: Failed reopen

- **WHEN** a reopen fails
- **THEN** an error starting "Reopen:" shows and the next ⇧⌘T tries the same entry

### Requirement: Plugin calls are bounded

A plugin SHALL hold at most 4 calls at once. A further call SHALL fail at once as busy, without
counting as a plugin failure. A due timer SHALL run before the plugin's queued events.

#### Scenario: Hung calls

- **WHEN** 4 calls of a plugin hang
- **THEN** a fifth returns busy at once, and the plugin is not paused for it

### Requirement: Diff tabs survive a restart

A document opened as a plugin pane view SHALL keep that view after a restart, and the diff
layout SHALL follow in every window. Opening the same file plainly SHALL show it as text.

#### Scenario: Restart

- **WHEN** the app restarts with a git diff tab open
- **THEN** the tab opens as a diff again

### Requirement: Agent bars belong to their plugin

The hook-off bar and the resume bar SHALL be drawn by the plugin that owns the agent, through a
pane-bar slot. With that plugin off, neither bar SHALL show.

#### Scenario: Claude plugin off

- **WHEN** the Claude plugin is off and a pane has a resume offer
- **THEN** no resume bar shows
