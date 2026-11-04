# Spec Delta

## ADDED Requirements

### Requirement: Context per side tab
Each side tab SHALL own a context of variables (name, value, kind, source, storage), pins,
links, text and files Claude edited, shared by all its tabs and shown in the context panel. Each item SHALL record its source
(typed, pasted, extracted, auto) and origin tab.

#### Scenario: Shared across tabs
- **WHEN** a variable is extracted in the Claude tab of PROJ-482
- **THEN** the shell tab of PROJ-482 can insert it with `⌘I`

### Requirement: Extract and pin a selection
`⌘E` SHALL save the selected text as a variable with a suggested, editable name. `⌘⇧P` SHALL
pin the selected text, or the whole message when nothing is selected. A pin from a transcript
pane SHALL store the entry id, character offsets and a text-quote selector (exact text, prefix,
suffix) to re-anchor it; a pin from a terminal pane SHALL store the text, the pane and the
buffer row range.

#### Scenario: Extract a PR URL
- **WHEN** the person selects `https://github.com/acme/api/pull/918` and presses `⌘E`
- **THEN** the name `pr_api` is suggested and Enter saves it to the side tab's context

#### Scenario: Pin part of a response
- **WHEN** the person selects "batches of 5,000 rows on a cursor over id" inside a longer assistant message and presses `⌘⇧P`
- **THEN** the pin shows only that phrase, and opening it scrolls the transcript to the message with the phrase highlighted

### Requirement: Extraction rules
The system SHALL run extraction rules on assistant entries and terminal output and SHALL add
each match once as an auto item: GitHub PR URLs, claude.ai artifact URLs and branch names in v1.

#### Scenario: Artifact URL seen twice
- **WHEN** an assistant entry contains the same artifact URL twice
- **THEN** the context holds one auto link for it

### Requirement: Items follow their tab
When a tab moves to another side tab, items it created (extracted, pinned, auto) SHALL move
with it, and items typed or pasted by hand SHALL stay.

#### Scenario: Move a tab
- **WHEN** a tab that extracted `pr_api` is dropped onto another side tab
- **THEN** `pr_api` appears in the target's context and leaves the source's

### Requirement: Variable storage modes
Each variable SHALL have a storage mode. Saved SHALL persist in SQLite. Session only SHALL live
in the daemon's memory, SHALL NOT be written to disk, SHALL survive an app restart, and SHALL
end with a reboot or when its side tab closes. Secret SHALL behave as session only, SHALL be
masked on screen, SHALL be left out of pins, copies, export, logs and hot exit, and MAY carry a
TTL. A side tab merge SHALL move secrets in the daemon and SHALL keep only their names in the
undo snapshot.

#### Scenario: Session-only variable after a quit
- **WHEN** the person quits and restarts the app while `from_date` is session only
- **THEN** `from_date` is still in the context, and the SQLite file holds no trace of it

#### Scenario: Secret expires
- **WHEN** `api_token` is a secret with a 1 hour TTL and an hour passes
- **THEN** the variable is removed from the context and from the daemon's memory

### Requirement: Insert a secret by reference
Inserting a secret into a pane SHALL paste `$NAME` only. The value SHALL reach shells through
`eval "$(terminescu var env)"` in the shell integration's `precmd`, and Claude's Bash commands
through the same line written to `CLAUDE_ENV_FILE` by the `SessionStart` hook. The value SHALL
NOT be typed into a pane. The insert dialog SHALL warn that pasting the raw value into a Claude
prompt stores it in Claude's transcript.

#### Scenario: Token for a curl call
- **WHEN** the person inserts the secret `API_TOKEN` in a shell pane and presses Enter
- **THEN** the visible text reads `$API_TOKEN`, the command gets the value, and the value is in neither the pane's scrollback nor the shell history

#### Scenario: Secret added after Claude started
- **WHEN** the person sets `API_TOKEN` as a secret while Claude already runs in the side tab, then asks Claude to run `curl -H "Authorization: Bearer $API_TOKEN" …`
- **THEN** Claude's Bash command gets the value and the transcript shows only `$API_TOKEN`
