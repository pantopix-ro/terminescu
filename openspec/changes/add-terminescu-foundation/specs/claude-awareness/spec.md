# Spec Delta

## ADDED Requirements

### Requirement: Hook bridge
The app SHALL install `terminescu hook` as a Claude Code command hook for `SessionStart`,
`UserPromptSubmit`, `PermissionRequest`, `PermissionDenied`, `PostToolUse` (every tool),
`PostToolUseFailure`, `Notification`, `Stop` and `SessionEnd`, with a 5 s timeout,
idempotently and without touching other hooks. The hook SHALL forward the hook JSON and
`TERMINESCU_PANE_ID` to the app's local socket. The app SHALL NOT start Claude, read or store
any Claude credential.

#### Scenario: Installer run twice
- **WHEN** the installer runs twice
- **THEN** `~/.claude/settings.json` holds one Terminescu entry per event and every other hook unchanged

### Requirement: Hook stays out of Claude's way
The hook SHALL exit 0 at once when `TERMINESCU_PANE_ID` is unset, SHALL always exit 0, SHALL
print nothing to stdout and SHALL give up on the socket after 100 ms.

#### Scenario: Claude outside Terminescu
- **WHEN** the person runs `claude` in iTerm, where `TERMINESCU_PANE_ID` is unset
- **THEN** every hook exits 0 without contacting the socket, and Claude shows no hook error

#### Scenario: App not running
- **WHEN** a Claude session in a Terminescu pane fires `Stop` while the app is quit
- **THEN** the hook exits 0 within 100 ms and Claude carries on

### Requirement: Session to pane mapping
On `SessionStart` the app SHALL record `session_id`, `transcript_path` and `cwd` against the
pane named by `TERMINESCU_PANE_ID`, and the pane and its tab SHALL show the `agent` icon until
`SessionEnd`.

#### Scenario: Claude typed in a shell
- **WHEN** the person types `claude` in a plain shell tab
- **THEN** the tab gains the agent icon and its title reads `claude · <cwd>`

### Requirement: Waiting detection
`PermissionRequest`, and a `Notification` whose `notification_type` is `elicitation_dialog`,
SHALL set the pane's tab to Waiting. `PostToolUse`, `PostToolUseFailure`,
`PermissionDenied`, `UserPromptSubmit` and `Stop` SHALL clear it. While a tab is Waiting the
app SHALL tail its transcript and clear Waiting on a user-rejected tool result or a
`turn_duration` line. A `Notification` of type `idle_prompt` or `permission_prompt` SHALL NOT
set Waiting.

#### Scenario: Permission prompt
- **WHEN** Claude asks to run a command in a background tab
- **THEN** the tab and its side tab show the amber needs-you marker and the status line count rises by one

#### Scenario: Permission approved
- **WHEN** the person approves the command and it runs
- **THEN** `PostToolUse` arrives, the amber marker clears and the tab shows Running

#### Scenario: Permission denied
- **WHEN** the person presses Esc on a permission prompt and no hook arrives
- **THEN** the transcript shows the rejected tool result, the amber marker clears and the tab shows Done

#### Scenario: Finished session left alone
- **WHEN** a session stops and Claude later sends an `idle_prompt` notification
- **THEN** the tab stays Done and no amber marker shows

### Requirement: Hook arrival order
Each hook posts on its own connection, so events MAY arrive out of order. A `permission_prompt`
notification SHALL NOT bring back Waiting, since it follows `PermissionRequest` by seconds and
can arrive after the prompt was answered. A tool event that arrives after `Stop` SHALL leave
the tab Done.

#### Scenario: Late permission notification
- **WHEN** the person approves a prompt and the `permission_prompt` notification arrives after
  `PostToolUse`
- **THEN** the tab stays Running

#### Scenario: Late tool result
- **WHEN** a `PostToolUse` event arrives after `Stop`
- **THEN** the tab stays Done

### Requirement: Transcript mirror
The app SHALL tail the session's JSONL with a byte-offset cursor, map lines to entries with
stable ids, and render them in a read-only transcript pane on request. In v1 the pane SHALL
show user and assistant messages with Pin and Extract on each message and on any selection;
tool calls, diffs and thinking SHALL be left out.

#### Scenario: Open a transcript pane
- **WHEN** the person splits a Claude pane and picks "transcript"
- **THEN** the new pane lists the session's entries, including ones from before the split

### Requirement: Resume after reboot
After a reboot a tab that held a Claude session SHALL show a "resume" button. Pressing it SHALL
type `claude --resume <session id>` and Enter into the tab's fresh shell. The app SHALL NOT
start `claude` without that press.

#### Scenario: Reboot with a Claude tab
- **WHEN** the Mac reboots while PROJ-482 has a Claude tab with session `3f9a`
- **THEN** the tab reopens with a login shell and a resume button, and nothing runs until the person presses it

### Requirement: Claude Code only in v1
The v1 runtime SHALL be Claude Code. Other agents SHALL map onto the same entry model in a later change.

#### Scenario: Another agent in a tab
- **WHEN** the person runs a different agent CLI in a tab
- **THEN** the tab behaves as a plain terminal with shell-integration statuses

### Requirement: Files Claude edits
Each `PostToolUse` event for a file-editing tool (Write, Edit, MultiEdit or NotebookEdit) SHALL add its file path to
the Files section of the side tab's context, with the path relative to the side tab's cwd, an
added or modified marker, the time and the origin tab, once per path. Clicking an entry SHALL
open the file in a text tab.

#### Scenario: Claude writes a proposal
- **WHEN** Claude in PROJ-482 writes `openspec/changes/add-audit-export/proposal.md` twice
- **THEN** the Files section lists that path once as added, with the latest time, and one click opens it
