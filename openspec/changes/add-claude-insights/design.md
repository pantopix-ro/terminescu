# Design: add-claude-insights

## Context

See proposal.md for why. After add-plugin-architecture the Claude plugin holds the hook
adapter, the transcript reader and the transcript pane. Hook payloads carry `session_id`,
`transcript_path`, `cwd`, `hook_event_name` and `permission_mode`; `PreToolUse` and
`PostToolUse` add `tool_name` and `tool_input` (`file_path` for Write and Edit); `SessionStart`
adds `source` and an optional `model` (code.claude.com/docs/en/hooks, read 2026-11-10).
`PreToolUse` blocks the tool until its hooks return. Today `hook_install.rs` leaves
`PreToolUse` out on purpose. Claude Code's own checkpoints in `~/.claude/file-history` have an
undocumented layout and skip Bash edits.

## Goals / Non-Goals

**Goals:** session insight from data Claude Code already writes or sends; baselines that never
write to the person's repository and never slow a turn.

**Non-Goals:** money costs; rewinding or restoring files; anything that types into Claude.

## Decisions

### D1. Baselines: a hybrid by row type
In a git repository the app snapshots the working tree at `SessionStart` (`source` startup or
fork; resume and clear keep the first baseline). Outside git, the hook copies each file before
Claude's first edit. Rejected: (a) hook copies everywhere, which misses Bash and formatter edits
in repositories, where most sessions run; (c) reading `file-history`, which is undocumented,
per prompt, and blind to Bash edits.

### D2. Working-tree snapshots in Terminescu's own object store
The snapshot runs in the app, off the hook, as
`GIT_INDEX_FILE=<store>/index GIT_OBJECT_DIRECTORY=<store>/objects
GIT_ALTERNATE_OBJECT_DIRECTORIES=<repo>/.git/objects git add -A && git write-tree`, with the
temp index first copied from the repository's index so unchanged files are not hashed again.
New blobs and trees land in Terminescu's store; the repository, its refs, index and objects are
untouched, and its `git gc` cannot prune them. Diffs run with the same environment. Rejected:
`git stash create` (no untracked files, objects in the repository) and a private ref (writes
refs). A `ready` marker file closes the snapshot.

### D3. The hook's first-touch copy
`hook_install` adds one `PreToolUse` entry with matcher `Write|Edit|MultiEdit|NotebookEdit`.
For those, `terminescu hook` reads `tool_input.file_path` (or `notebook_path`) and, when no
copy exists for that session and path and the session's `ready` marker is absent, makes an APFS
`clonefile` copy into `<store>/<session>/files/<sha256 of path>`; a file over the cap or on
another volume gets a marker instead of a copy. It covers repository sessions until their
snapshot is ready, too: a per-file copy wins over the tree snapshot. Every step is bounded: one
`stat`, one `clonefile`, no reads of file contents. The hook still exits 0, prints nothing and
returns within 100 ms; on any error it skips the copy.

The CLI must not depend on a plugin (add-plugin-architecture D11), so it knows no Claude
payload. The plugin installs the entry as `terminescu hook --baseline-session /session_id
--baseline-path /tool_input/file_path --baseline-path /tool_input/notebook_path --baseline-cwd
/cwd`: JSON pointers into the event. With them the hook copies and forwards nothing. The store
layout and the copy live in `terminescu-plugin-api` (`baseline`), which both use, and so does
the timed program runner the git plugin had (`process`).

### D4. Context meter from the transcript
The transcript reader already tails waiting and running sessions. It now keeps the last
assistant entry's `message.usage` and `message.model` per session. Used tokens are
`input_tokens + cache_creation_input_tokens + cache_read_input_tokens`. The window comes from
Settings › Claude per model, else 200,000. The meter is a `meter` decoration on the tab.

The core's transcript tail feeds the adapter's `on_line`, which the plugin cannot see. So the
plugin tails the transcript itself with the plugin-api `LineTail`: from the hook's
`transcript_path`, else `PluginCx::agent_log`, on a one-second timer while any of its panes
runs a session. It sets the meter, the chips, the subtitle and the badge on the pane's tab,
and only when they change. Entries of subagents (`isSidechain`) do not count.

### D5. Todo mirror
The reader keeps the latest `tool_use` named `TodoWrite` (input `todos[]` with `content`,
`status`, `activeForm`). Another tool name or shape is ignored, so a Claude Code change hides
the section instead of breaking it. The panel section is the plugin's.

### D6. Session history in row data
On `SessionStart` and `SessionEnd`, and on the first `UserPromptSubmit` (for the prompt's first
line, at most 80 characters), the plugin updates `plugin_side_tab_data` key `sessions` for the
pane's row: newest first, at most 50. Resume uses `PluginCx` "run a command line in a new
login-shell tab without Enter".

### D7. Signals from hooks
Waiting reason from `PermissionRequest` `tool_name` and `tool_input` (`command`, `file_path`,
`url`, else the first string field). Model from `SessionStart.model`, mode from
`permission_mode` on every event. Compaction from `PreCompact` until the next `SessionStart`
with source `compact` or `Stop`. Subagents from `SubagentStart` and `SubagentStop` when Claude
Code sends them; `hook_install` adds the three events.

The waiting reason is a field of the shared agent type (`Agent.reason`), which the adapter
sets on `PermissionRequest` and clears once the agent stops waiting. The core shows it
without knowing Claude. The notification keeps the row and the tab as its title and puts the
reason in the body, so macOS shows "PROJ-482 › api" over "Bash: npm publish --access public".
A path inside the session's folder shows relative to it.

## Risks / Trade-offs

- [A big repository makes the snapshot slow] → It runs in the app's plugin worker, with the
  copied index; the hook's first-touch copies cover edits until `ready` exists. Snapshots over
  30 seconds stop and fall back to first-touch copies for that session.
- [Ignored files such as `.env` get no baseline in git rows] → Correct by design: they are not
  the person's tracked work; the hook copy still covers an Edit to them.
- [Baselines hold copies of the person's files] → 0600, inside Application Support, 7 days,
  500 MB cap, removed by Uninstall Helpers. Secrets stored in variables are never involved.
- [Claude Code changes a payload or transcript shape] → Each reader ignores unknown shapes; the
  feature hides rather than breaks, and `app.log` notes the first miss per session.
- [The `PreToolUse` hook adds latency to every edit] → Measured in a test: under 15 ms on a
  2026 MacBook for a 5 MB file; the contract stays 100 ms.

## Migration Plan

Hook entries change on the next launch through the existing idempotent install. No database
migration. Removing the plugin leaves `baselines/` for Uninstall Helpers.
