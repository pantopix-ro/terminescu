# Proposal: add-claude-insights

## Why

Terminescu knows when a Claude session waits, but not how much context it has left, what it
changed, what it plans next, or which earlier sessions a row had. People find out by asking
Claude, scrolling, or running `git diff` and guessing which lines were theirs. The Claude plugin
from add-plugin-architecture can show all of this from the hook events and the transcript it
already reads, without a token and without starting a turn.

## What Changes

- A context meter on each tab with a Claude session: context used out of the model's window,
  from the transcript's token usage, amber from 80%.
- "What Claude changed": per session, the files Claude changed and a diff of each against how
  it was before the session. In a git repository the app snapshots the working tree when the
  session starts, so Bash and formatter edits show too. Elsewhere the hook keeps a copy of each
  file before Claude's first edit to it.
- A todo mirror: Claude's current todo list in the context panel, from its todo tool calls.
- Session history per row: the Claude sessions the row's tabs ran, with their first prompt and
  when they ended. "Resume…" opens a login-shell tab and types `claude --resume <id>` without
  Enter.
- The waiting reason: the notification and the amber tip name the tool and command Claude
  waits on, as "Bash: npm publish".
- Chips for the session's model and permission mode (plan, accept edits, bypass), and a
  "compacting" subtitle while Claude compacts.
- A running subagent count on the tab, when Claude Code reports subagents.
- Open memory files: the repository's `CLAUDE.md`, the user's `~/.claude/CLAUDE.md` and the
  session's transcript folder, from the row menu and the palette.
- Settings › Claude: the meter threshold, the model window override, the snapshot size cap and
  retention.

## Capabilities

### New Capabilities

- `claude-insights`: context meter, session changes and snapshots, todo mirror, session history
  and resume, waiting reason, model and mode chips, compaction and subagent signals, memory
  files.

### Modified Capabilities

None. `claude-awareness` (in add-terminescu-foundation) keeps its requirements. The hook gains
a `PreToolUse` entry for edit tools only, within the hook's existing contract: it exits 0,
prints nothing and returns within 100 ms.

## Impact

- `crates/plugin-claude`, `packages/plugin-claude`, and `crates/cli` (`hook.rs`: the
  first-touch copy).
- Snapshot store under `~/Library/Application Support/Terminescu/baselines`, files 0600,
  7-day retention, 500 MB cap.
- Runs the person's `git` for working-tree snapshots, with Terminescu's own object directory,
  so the repository is not written to.
- Per-row session history in `plugin_side_tab_data` (migration 0013). No new migration.
- Depends on add-plugin-architecture and add-git-plugin (the diff tab pane view).

Out of scope: reading Claude Code's own `file-history` checkpoints (an undocumented format);
token costs in money; anything that types into Claude or presses Enter for it.
