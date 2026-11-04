# Spike 0.5: hook events

Run on 2026-11-04 against Claude Code 2.1.318, model Haiku, `--permission-mode default`, in a
throwaway git repo with invented files. A logging hook was registered for every event that D34
names. It wrote stdin plus a timestamp to a file, only when `TERMINESCU_PANE_ID` was set.
Fixtures with paths, ids and session ids replaced live in `crates/core/tests/fixtures/hooks/`.
`sequences.json` there holds the measured order and timing for each case below.

## Results

| Case | Hooks, in order | What it means for D34 |
|---|---|---|
| Read-only command (`wc -l`) | `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop` | Claude auto-approves read-only commands; no prompt, no Waiting |
| Prompt approved after 30 s | `PreToolUse`, `PermissionRequest` (+0.05 s), `Notification permission_prompt` (+6.0 s), `PostToolUse`, `Stop` | `PermissionRequest` is the fast Waiting signal |
| Prompt approved within 2 s | `PreToolUse`, `PermissionRequest`, `PostToolUse` | No `Notification` at all when the person answers before 6 s |
| Prompt denied (Esc, or "No") | `PreToolUse`, `PermissionRequest`, `Notification permission_prompt`, then nothing | No hook reports the denial. No `Stop` either |
| Idle after a turn | `Stop`, then `Notification idle_prompt` 60.2 s later | Fires once per turn, message "Claude is waiting for your input" |
| `⌃G` editor | none | Editor runs outside the hook system |
| `/exit` | `SessionEnd` with `reason: "prompt_input_exit"` | Drops the agent icon |

### Denial

A denied prompt leaves the tab Waiting forever if the app listens to hooks only. The transcript
records the denial within the same second:

1. a `user` line whose `tool_result` has `is_error: true`, `toolUseResult: "User rejected tool use"`
   and `toolDenialKind: "user-rejected"`;
2. a `user` text line `[Request interrupted by user for tool use]`;
3. a `system` line with `subtype: "turn_duration"`.

So the core clears Waiting when the transcript tail shows `toolDenialKind` or a
`turn_duration` line after the `PermissionRequest`, and also on the next `UserPromptSubmit`.
`PermissionDenied` never fired here; it most likely covers denials made by auto mode.

### `⌃G`

Claude runs `$VISUAL` when both `VISUAL` and `EDITOR` are set, passes one argument (a
`claude-prompt-<uuid>.md` file under `$TMPDIR`), and blocks until the editor exits. On exit the
prompt box holds the file's text and shows "Edited in the external editor (via visual)".
`terminescu edit --wait` must therefore stay in the foreground until the text tab closes, and
the shell integration sets `VISUAL`, not only `EDITOR` (D27).

### File tools

`Write` and `Edit` fire under those names with `tool_input.file_path`. Write's
`tool_response.type` is `"create"` for a new file. Edit's `tool_response` carries `filePath`,
`oldString`, `newString`, `originalFile`, `structuredPatch`, `userModified` and `replaceAll`.
`Read` fires `PreToolUse` and `PostToolUse` too, so the Files section (D30) filters by tool name.
`MultiEdit` and `NotebookEdit` did not appear; the filter keeps their names in case they come back.

### Payloads

- Every event: `session_id`, `transcript_path`, `cwd`, `hook_event_name`.
- `SessionStart`: also `model`, `source` (`startup`), `scratchpad_dir`.
- `Stop`: also `last_assistant_message`, `permission_mode`, `prompt_id`, `stop_hook_active`,
  `background_tasks`, `session_crons`, `effort`.
- `Notification`: `notification_type` and `message` ("Claude needs your permission" carries no
  tool name, so the status text comes from the preceding `PermissionRequest`).

## Other notes

- The workspace trust dialog on a new folder defaults to "No, exit". The first `claude` in an
  untrusted folder needs a keypress, and no hook fires before that answer.
