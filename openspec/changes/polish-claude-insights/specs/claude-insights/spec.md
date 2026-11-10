## ADDED Requirements

### Requirement: Lead pane per tab

A tab with two Claude panes SHALL show the decorations of the pane with the latest hook event.
When that pane's session ends or the pane closes, the other pane's decorations SHALL show.

#### Scenario: Two panes

- **WHEN** pane A then pane B in one tab get hook events
- **THEN** the tab shows B's model chip
- **WHEN** B's session ends
- **THEN** the tab shows A's model chip

### Requirement: Hidden Claude sections

A panel section SHALL be able to hide itself. The Claude plan and change sections SHALL hide in a
tab with no Claude session and show again when one starts.

#### Scenario: Plain shell tab

- **WHEN** the shown tab has no Claude session
- **THEN** neither "Claude's plan" nor "What Claude changed" shows

### Requirement: Per-file baseline limit

Settings › Claude SHALL offer the largest file a baseline keeps, 1 to 100 MB, default 5. The hook
and the diff SHALL use it.

#### Scenario: Lower limit

- **WHEN** the limit is 1 MB and Claude edits a 2 MB file
- **THEN** the file has no copy and its diff says it is too large

### Requirement: Snapshots run no filters

A snapshot SHALL run no clean or process filter and SHALL write nothing into the repository.

#### Scenario: Filter driver

- **WHEN** a repository has a clean filter for `*.bin`
- **THEN** the snapshot holds the file's own bytes and the filter never runs

### Requirement: Idle tail

The transcript tail SHALL read every second while a session works and every 10 s otherwise.

#### Scenario: Turn ends

- **WHEN** a session's `Stop` arrives and no other session works
- **THEN** the next tail tick is 10 s away

### Requirement: Safe waiting reason

A waiting reason SHALL name a shell command only by its program and subcommand, and a URL without
its query. A question SHALL show its text.

#### Scenario: Bash with a token

- **WHEN** Claude asks to run `API_TOKEN=abc ./deploy.sh prod`
- **THEN** the reason reads `Bash: deploy.sh prod`, without the token

#### Scenario: Question

- **WHEN** Claude asks "Which branch?"
- **THEN** the pane waits with the reason "Which branch?"
