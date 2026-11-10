## Purpose

The Claude plugin shows what each Claude Code session is doing and has done: context left,
files changed, its todo list, past sessions and why it waits, from hook events and transcripts,
with no token and no turn started by Terminescu.

## ADDED Requirements

### Requirement: Context meter
A tab with a Claude session SHALL show a meter of context used out of the session model's
context window, from the token usage of the transcript's latest assistant entry (input, cache
creation and cache read tokens). The meter SHALL update within 2 seconds of a new entry, turn
amber at the threshold (80% by default), and its tip SHALL give the tokens used, the window and
the model.

#### Scenario: Close to full
- **WHEN** the latest entry used 172,000 tokens of a 200,000-token window
- **THEN** the tab's meter reads 86% in amber and its tip says "172k of 200k tokens"

### Requirement: Context window per model
Settings › Claude SHALL set the meter threshold and a context window per model. Without a
window set, a model's window SHALL be 200,000 tokens.

#### Scenario: A 1M window
- **WHEN** the person sets 1,000,000 tokens for the session's model and the entry used 172,000
- **THEN** the meter reads 17% and is not amber

### Requirement: What Claude changed
For each Claude session the plugin SHALL list the files that changed since the session
started, with lines added and removed, and SHALL open any of them as a diff against its content
before the session. A file over the size cap (5 MB by default) SHALL show "Too large to
compare". Taking a baseline SHALL never write to the repository and SHALL not delay Claude's
turn: the hook part returns within 100 ms.

#### Scenario: Your own changes stay yours
- **WHEN** `src/retry.ts` already had the person's uncommitted edits when the session started
- **THEN** the diff shows only the lines that changed after the session started

### Requirement: Baselines in a repository
In a git repository the baseline SHALL be a snapshot of the working tree taken when the
session starts, including untracked files that git does not ignore, so edits by Bash commands
and formatters show.

#### Scenario: Changes in a repository
- **WHEN** a session in `shop-api` edits `src/retry.ts` with its Edit tool and runs `prettier
  --write src/`, which reformats `src/client.ts`
- **THEN** the session's changes list both files, and `git status` in `shop-api` shows nothing
  Terminescu wrote

### Requirement: Baselines outside git
Outside git, the baseline SHALL be a copy of each file taken before Claude's first edit to it
with its edit tools. Edits by Bash commands SHALL not show there, and the list SHALL say so.

#### Scenario: Outside git
- **WHEN** a session in `~/notes` edits `todo.md` with its Edit tool and changes `log.txt` with
  `sed`
- **THEN** the list shows `todo.md` with its diff and says that Bash edits are not tracked here

### Requirement: Baseline retention
Baselines SHALL live in Terminescu's application support folder with owner-only permissions,
SHALL go 7 days after their session ends, and SHALL be removed oldest first when they pass
500 MB. Both limits SHALL be set in Settings › Claude. Uninstall Helpers SHALL remove them.

#### Scenario: Over the cap
- **WHEN** baselines reach 520 MB
- **THEN** the oldest ended sessions' baselines go until the total is under 500 MB, and running
  sessions keep theirs

### Requirement: Todo mirror
The context panel SHALL show the Claude session's current todo list for the active tab, from
its latest todo tool call in the transcript, with each item's state (pending, in progress,
done). An unknown todo tool shape SHALL be ignored.

#### Scenario: Claude plans
- **WHEN** Claude writes a todo list of 4 items and marks the first in progress
- **THEN** the panel shows the 4 items with the first marked in progress, within 2 seconds

### Requirement: Session history and resume
Each row SHALL keep the Claude sessions its tabs ran: the session id, the first prompt's first
line, the start and end times, and the folder. The row menu and the palette SHALL offer
"Resume…", which lists them newest first; picking one SHALL open a new login-shell tab in the
session's folder and type `claude --resume <id>` without pressing Enter. History SHALL go with
its row.

#### Scenario: Resume yesterday's session
- **WHEN** the person picks "Resume…" in row `PROJ-482` and selects yesterday's session
- **THEN** a new tab opens in that session's folder with `claude --resume <id>` typed at the
  prompt and no Enter

### Requirement: Waiting reason
When a Claude session waits on a permission, the notification and the amber row's tip SHALL
name the tool and its main argument, shortened to 60 characters, as "Bash: npm publish" or
"Edit: src/retry.ts". A wait with no tool SHALL say "Waiting for you".

#### Scenario: A Bash permission
- **WHEN** Claude asks to run `npm publish --access public` while another app is in front
- **THEN** the notification reads "PROJ-482 › api — Bash: npm publish --access public"

### Requirement: Model, mode and progress signals
A tab with a Claude session SHALL show a chip for the model and, when not the default, the
permission mode (plan, accept edits, bypass). While Claude compacts, the tab's subtitle SHALL
read "compacting". When Claude Code reports subagents, the tab SHALL show how many run.

#### Scenario: Plan mode
- **WHEN** the session switches to plan mode
- **THEN** the tab shows a "plan" chip beside its model chip

### Requirement: Memory files
The row menu and the palette SHALL open the repository's `CLAUDE.md` (from the shown pane's
repository root, else its cwd) and the user's `~/.claude/CLAUDE.md` in text tabs, and SHALL
reveal the session's transcript folder in Finder. A missing `CLAUDE.md` SHALL open as a new
file at that path, saved only on `⌘S`.

#### Scenario: Open project memory
- **WHEN** the person picks "Open CLAUDE.md" in a row whose tab is in `shop-api/src`
- **THEN** `shop-api/CLAUDE.md` opens in a text tab beside the focused pane
