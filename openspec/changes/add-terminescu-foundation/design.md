# Design: add-terminescu-foundation (Terminescu)

## Decisions

### D1. A terminal manager, not an orchestrator
The person drives every session. The app groups terminals, remembers what came out of them,
and says which one needs attention. It never starts a Claude turn on its own. Rejected: the
flow engine of the first draft (DBOS runs, breakpoints, confidence gates), because daily use
showed the value sits in grouping, prompts and extracted facts, and a flow engine is a large
system to build before any of that ships. Flows can return as `add-flows` on top of this model.

### D2. Claude runs as the unmodified `claude` CLI; the Agent SDK package is never used
Anthropic's legal page limits subscription OAuth to Claude Code and native Anthropic apps and
forbids collecting or storing Claude.ai credentials. The app starts nothing on the person's
behalf: they type `claude` in a shell, or insert a command from the palette. The app never
reads or stores a token. Rejected: `@anthropic-ai/claude-agent-sdk`, named in the policy.

### D3. Window, side tab, tab, pane
A window is an OS window. Its sidebar lists side tabs; each side tab is a terminal window
living inside the app, holding 1 to N tabs shown across the top like browser tabs. A tab is a
tree of splits whose leaves are panes of three kinds: `terminal` (a pty), `transcript` (a
rendered Claude session) or `text` (a text editor, D27). A side tab holds any mix, and a
side tab of text tabs only is fine. A side tab with one tab draws as a single row; with two or more as a group
that expands to list its tabs. Rejected: tmux-style sessions and windows as the user-facing
model, because browser tabs plus groups are the model people already drag around.

### D4. One context per side tab
A side tab owns a context: variables (name, value, kind, source), pins, links and text. Every
tab in the side tab reads and writes the same context. Items carry an origin tab. Rejected:
a context per tab, because one ticket spans several tabs (Claude, dev server, a shell) and the
facts belong to the ticket.

### D5. Drag behaves like a browser
Reorder inside the tab bar; drop on a sidebar gap to make a new side tab there; drop onto a
side tab row to move the tab into it; drop outside every window to open a new window with a
default side tab holding the tab. Drop a side tab onto another to merge: tabs move, lists
(pins, links, text) concatenate, variables union. A variable present in both with different
values is a clash: keep the target's value, or keep both and rename the source's
(`{{branch_477}}` by default). No clash means no dialog, only a toast with undo for 10 s. When
a tab moves alone, items it created (extracted, pinned, auto) move with it; items typed or
pasted by hand stay. Every drag has a palette equivalent.

### D6. Tauri 2 with a Rust core; the UI talks to a `Host`
`Terminescu.app` is a Tauri 2 app: a native macOS binary, real `⌘` shortcuts and multi-window
support. The Rust core runs in the app process and is the single source of truth for windows,
side tabs, layout, contexts and statuses, so a tab can move between windows. Each OS window is
a webview running React 19, Vite, TypeScript and xterm.js with the WebGL addon. Windows send
commands (`MoveTab`, `Split`, `SetStatus`, `Extract`, `Insert`) through the `Host` interface;
the core applies them, persists, and broadcasts patches to every window, each holding a Zustand
store. v1 implements `Host` over Tauri IPC; the remote view (D22) speaks the same protocol over
WebSocket. The app stays alive as a menu-bar app when every window closes. Rejected: Electron
(heavier, the pty would still live in Node), staying in the browser (`⌘W`, `⌘T`, `⌘1` belong
to the browser).
### D7. A pty daemon owns every shell
`terminescu-ptyd` is a small Rust process with a stable, versioned Unix-socket protocol. It
spawns each pane's login shell with `portable-pty` and `TERMINESCU_PANE_ID` in its environment,
keeps a raw scrollback ring per pane (10,000 lines by default), reads the foreground process
with `tcgetpgrp`, parses OSC 133 and OSC 7, and holds the in-memory store for session-only and
secret variables (D21). It survives app quit and app upgrades; a reboot ends it (D23). How it starts and upgrades is
D32; its protocol starts at version 1. Rejected:
tmux as the backend, because xterm.js then shows tmux's screen instead of native scrollback and
selection, and tmux's prefix key fights the person's own tmux. A tmux-backed pane can return as
an option.
### D8. New tabs start the login shell
`⌘T` opens `$SHELL -l` in the current pane's cwd. Claude is something the person runs in a
shell. The palette offers "New tab with claude" for one keystroke less.

### D9. Shell integration for command state
The daemon injects a small zsh, bash and fish integration that emits OSC 133 (prompt, command
start, command end with exit code) and OSC 7 (cwd). Tab titles, the running and failed states
and "new tab in this cwd" come from it. It also reports the command line of each command
(an `OSC 633;E`-style sequence from `preexec`), which status rules match on (D15), because OSC 133
carries no command text. It runs after the person's rc files, so it can set `VISUAL` and `EDITOR`
when the opt-in editor setting is on (D27), and its `precmd` loads session and secret variables
(D35). Without it a pane falls back to the process name. Rejected: setting `EDITOR` in the spawn
environment, because `.zshrc` exports override it and git reads `VISUAL` first.

### D10. Claude awareness through command hooks
`terminescu hook` is registered as a command hook in `~/.claude/settings.json`
for `SessionStart`, `UserPromptSubmit`, `PermissionRequest`, `PermissionDenied`, `PostToolUse`,
`PostToolUseFailure`, `Notification`, `Stop` and `SessionEnd`; D34 maps each to a state. Claude runs
hooks in its own environment, so the hook sees `TERMINESCU_PANE_ID`; it posts the hook JSON plus that
id to the app's socket. The app maps `session_id` and `transcript_path` to the pane. The
installer is idempotent and leaves other hooks alone. `PostToolUse` matches every tool,
because it also clears Waiting; the core keeps the file-editing tools for the Files section
(D30). Rejected: HTTP hooks, because they carry
no environment and cannot name the pane.

### D11. Every runtime maps onto one normalized entry model
`Entry` with kinds user, assistant, tool_use, tool_result, thinking, system, error, and a
stable id per session, read by tailing the transcript JSONL with a byte-offset cursor. The
transcript pane, pins and extraction consume only entries.

### D12. Pins point at transcript entries or a selection in them
A pin holds a whole entry or any selected text. From a transcript pane it stores side tab,
origin tab, session, entry id, character offsets and a text-quote selector (exact text, 40
characters of prefix and suffix) that re-anchors the pin if offsets drift; from a terminal pane
it stores the text, the pane and the buffer row range. Each pin keeps the excerpt and a note.
Pins survive compaction because they reference the transcript, not the context window.
`{{pins}}` renders the side tab's pins into a prompt.
### D13. Extract, insert, send
`⌘E` turns a selection into a variable, with a name suggested from the content (`pr_api` for
a GitHub PR URL in the `api` repo, `branch` for a branch name). Extraction rules run on
assistant entries and on terminal output: GitHub PR URLs, claude.ai artifact URLs and branch
names in v1, user-defined regex rules later. `⌘I` opens a palette of prompts and variables;
the chosen text is written to the focused pane as a bracketed paste and Enter is never sent
unless the person picks "insert and send". "Send to…" pastes a selection into another tab.

### D14. A prompt library is a first-class screen
Prompts carry the daily value, so the library is its own screen: search over name, tags and
body, tags instead of folders, last used and use count, duplicate, archive, versions with
restore. Prompts are versioned Markdown with `{{variables}}` and images. Two entry points save
without leaving the work: "Save as prompt" in the insert dialog after editing, and on any
message the person sent in a transcript pane. Saving offers to replace text matching a
context variable's value with `{{name}}`. The library edits prompts in the same text editor as
text tabs (D29).

### D15. Statuses: built-in, global, local, dynamic
Three tiers. Built-in auto statuses (Running, Waiting, Done, Failed, Idle) come from hooks and
shell integration; Settings renames and recolors them, nothing deletes them. Global statuses
live in Settings > Statuses with label, accent, background and order. Local statuses are saved
on one side tab or tab and offer "make global". A status turns dynamic three ways: rules in
Settings (`pnpm test` exits non-zero sets `Tests red`, Claude idle 30 min sets `Stale`),
`terminescu status "Deploying" --color cyan` from any shell, which Claude can run too, and an
expiry (`--until-exit` or `--for 20m`). Precedence: manual, then rule, then auto. A side tab's
auto status is its most urgent tab's. The amber needs-you marker shows over any status whenever
a Claude session inside waits. Deleting a status in use sends those tabs back to auto. Accents
are six hues (`blue cyan green magenta red fg`), backgrounds five tints; the sidebar sorts by
hand (default), or by background, status or name.
### D16. Terminal-first visual system with pixel trim
Chosen on 2026-11-03, kept on 2026-11-04 after a flatter all-terminal variant (1px lines, no
pixel font, text glyphs for icons) read as too plain. Dark only in v1, every color a token, the
xterm theme generated from the same tokens. Geist Mono for everything read, Silkscreen only for
short uppercase labels. Notched frames, hard shadows with no blur, an own 8x8 icon set, block
strips with one cell per tab. Amber means "needs you", focus and drop targets, and is never
offered as an accent so it keeps that meaning. People sort their work with accents and `tint-*`
backgrounds. Stepped motion, off under reduced motion; the chime is off by default.
### D17. Plain keys go to the pane; app actions are `⌘` chords
Shells and Claude don't read `⌘` on macOS, so every app action is a `⌘` chord and the
terminal keeps every other key: `⌘K` palette, `⌘T` tab, `⌘N` side tab, `⌘D` and `⌘⇧D` split,
`⌘W` close, `⌘1`-`⌘9` tab, `⌃⌘1`-`⌃⌘9` side tab, `⌘I` insert, `⌘E` extract, `⌘⇧P` pin,
`⌘⇧S` status editor, `⌘⇧A` jump to the oldest waiting session, `⌥⌘T` text tab, `⌘O` open
file. The app's chord wins in every pane kind, text panes included: `⌘D` splits, `⌘I`
inserts, `⌘K` opens the palette, `⌘E` extracts, and CodeMirror's select-next moves to `⌃⌘G`.
Every shortcut is editable in Settings › Keys in v1, which flags conflicts. Rejected: the first draft's
Esc Esc plus single letters, which made the person leave the terminal before every action.

### D18. SQLite in the core, an owner on every row
`rusqlite` in the Rust core with embedded migrations. Tables: `owners`, `windows`, `side_tabs`,
`tabs`, `panes` (layout tree as JSON per tab), `context_items`, `pins`, `prompts`,
`prompt_versions`, `commands`, `statuses`, `status_rules`, `extraction_rules`, `sessions`,
`documents`, `context_files`, in migration `0001_init.sql`. `pairing_tokens` waits for
`add-remote-view`. Every table carries `owner_id`; v1 creates one local owner on first run.
Session-only and secret variables never reach SQLite: they are a separate store type in the
core and live in the daemon (D21). Rejected: Postgres in Docker, too heavy for a desktop app.
### D19. Process layout and build
```
 Terminescu.app (Tauri 2)                       terminescu-ptyd
 ┌──────────────────────────────────┐  unix    ┌──────────────────────────┐
 │ webview per window               │  socket  │ pty per pane (login sh)  │
 │  React, xterm.js, Zustand        │◀────────▶│ scrollback ring, OSC 133 │
 │        ▲ commands  │ patches     │          │ foreground process       │
 │        │ (Host)    ▼             │          │ session + secret vars    │
 │ Rust core: state, SQLite,        │          └──────────────────────────┘
 │  statuses, rules, extraction     │◀──── terminescu hook  (Claude Code hooks)
 │  hook socket, later remote view  │◀──── terminescu status / var  (any shell)
 └──────────────────────────────────┘
```
The `terminescu` CLI and the daemon ship inside the app as Tauri `externalBin` sidecars.
Libraries: `react-resizable-panels` for splits, `cmdk` for the palette, `dnd-kit` for drags
inside a window, Zustand; `rusqlite`, `notify` for JSONL tailing, `vte` to strip ANSI before
extraction, `zeroize` for secrets, `axum` later for the remote view. Layout: a Cargo workspace
(`core`, `ptyd`, `cli`, `protocol`) and a pnpm workspace (`apps/desktop`, `packages/ui`,
`packages/host`). Tests: Rust unit tests for merge, statuses and rules, ptyd integration tests
against real shells, Playwright on the Vite build with a mock `Host`. Signing and notarization
come with selling.

### D20. Reattach is one mechanism, and the window looks native
Moving a tab to another window, restarting the app and opening the remote view all reattach the
same way: a new xterm instance attaches to the pane id, the daemon replays the ring, then sends
a resize so TUIs such as `claude` redraw. If raw replay proves wrong, the fallback is a headless
screen model in the daemon (`alacritty_terminal`, as Zed uses). The window uses
`titleBarStyle: Overlay` with a hidden title, so the tab bar sits in the title bar like Chrome
or Ghostty, with the traffic lights positioned to match. App actions are native menu items with
`⌘` accelerators, so chords win over the webview, and xterm's `attachCustomKeyEventHandler`
passes `⌘` chords up. Cross-window drags: the core tracks the global cursor during a drag
(Tauri `cursor_position`), tells the window under it which drop target is hot, and moves a small
transparent always-on-top ghost window with the cursor; release outside every window opens a new
`WebviewWindow` there with a default side tab.

### D21. Variables have a storage mode
Saved (default) lives in SQLite. Session only lives in the daemon's memory, never on disk,
survives an app restart, and ends with a reboot or when its side tab closes; the UI draws it
with a dashed outline. Secret is session only plus masked, left out of pins, copies, export, logs
and hot exit, with an optional TTL. A side tab merge moves its secrets in the daemon; the undo
snapshot holds their names only. A secret inserts as `$NAME`, and D35 gets the value into the
environment without crossing the screen. Inserting a raw value into a Claude prompt still lands
in Claude's own JSONL; the insert dialog says so for secrets.

### D22. The web app is a remote view of the Mac
Later, the core serves the same commands, patches and pty streams over WebSocket with `axum`,
behind a pairing token, reached over Tailscale or an SSH tunnel. The browser runs the same
React bundle with a WebSocket `Host`. No cloud, no sync; each pairing token maps to an
`owner_id`.

### D23. Reboot recovery
After a reboot the app restores every window and layout with fresh login shells in each pane's
last cwd. Launcher side tabs rerun their commands (D24). A tab that held a Claude session shows
a "resume" button; pressing it types `claude --resume <id>` and Enter. The app never starts
`claude` without that press.

### D24. Commands and launchers
Commands are saved shell lines with `{{variables}}`, kept per side tab in its context or
globally in the library, and listed in a Commands section of the sidebar. "Run" pastes and
sends Enter; "Insert" pastes without Enter; targets are a new tab, the focused pane or a new
side tab. A launcher side tab is defined by its commands: it runs each in its own tab when
opened or after a reboot, offers restart, and derives its status from those processes
(`2/2 up`, `1/2 up`).

### D25. Smaller v1 choices
The transcript pane ships in P1 in a minimal form: read-only user and assistant messages with
Pin and Extract per message and per selection; tool calls, diffs and thinking come in P2.
Settings in v1: Statuses, Commands, Variable defaults, Extraction rules (read-only), Shell and
Keys (editable, with conflict detection, D17). A macOS notification fires when a Claude
session waits while the app is in the background, on by default with no sound. Only Claude
Code is supported in v1; the entry model (D11) stays ready for Codex or OpenCode.

### D26. Panels collapse and scroll
`⌘\` cycles the context panel (Context, Pins, Prompts) between pinned and auto-hide. Auto-hidden,
hovering a 6px edge at the right of the window for 150 ms slides it in as an overlay over the
panes, so terminals never resize, and it slides out when the pointer leaves. The sidebar, each
pane and the context panel scroll on their own; the tab bar scrolls sideways on overflow.

### D27. Text tabs give Terminescu the abilities of a text editor
Text editors matter in a working day: prompts, notes, commit messages and config get edited
by hand next to the terminals. A text tab is an editor, and it stays one: no file tree, no
project search, no language servers, no git UI. Openers:
- `⌘T` or `+` opens a terminal tab, as before; `⌥⌘T` or `+ ▾ › Text` opens an Untitled text tab.
- `⌘O` searches files under the side tab's cwd; `⌘O` again opens the macOS open dialog.
- `⌘`-click on a file path in terminal output opens it in the same side tab, the path resolved
  against the pane's OSC 7 cwd.
- `terminescu edit <file>` in a shell opens it in the side tab of the calling pane
  (`TERMINESCU_PANE_ID`).
- Clicking a text variable or a prompt in the context panel opens it; saving updates the
  variable or creates a new prompt version.
- Dropping a file from Finder on the tab bar opens it.
Holding `⌥` with any opener splits the focused pane instead of adding a tab. Settings › Shell
offers `VISUAL` and `EDITOR` set to `terminescu edit --wait` (opt-in, set by the shell integration, D9): a program asking for an editor (git commit,
Claude Code's edit-prompt-in-editor) gets a text pane split beside the pane that asked, which
shows the amber needs-you marker while the process waits. `⌘↵` saves and returns; closing the
pane ends the wait. Saving behaves like a text editor: `⌘S` saves, an Untitled buffer opens the
macOS Save dialog, Save As sits in the File menu and the palette, and closing with unsaved
changes asks Save, Don't save or Cancel. Claude gets no special access to text tabs.
Rejected: a file manager with a tree, because it grows the app into an IDE.

### D28. Documents live in the core
Every text pane edits one document with a source: a file, an Untitled buffer, a context
variable, a prompt version or an editor request; new sources (a PR description) plug in later.
Documents live in the Rust core the way shells live in `terminescu-ptyd`: the editor sends its
changes every 300 ms, a tab moved to another window reloads from the core, and undo history
resets on that move (accepted for v1). Unsaved changes and Untitled buffers survive quit and
restart (hot exit) in SQLite with `owner_id`. A document marked session only skips hot exit
and never reaches disk. A file changed on disk reloads when the buffer is clean; otherwise a
"changed on disk" bar offers Reload or Keep mine. Files above 20 MB open read-only and binary
files are refused. Text tab statuses: Idle, Changed on disk, and Waiting (amber) for an editor
request; unsaved changes show as a dot on the tab, not a status color.

### D29. CodeMirror 6 as the editor
Text stays byte-exact (spacing, line endings), because Claude and other tools edit the same
files. Rich display comes from decorations over plain text, the approach Obsidian's live
preview proves. Features, Terminescu's own included, ship as CodeMirror extensions. v1:
Markdown and common languages (language data lazy loaded), soft wrap, `⌘F` find and replace,
undo, multiple cursors, `{{variable}}` chips with autocomplete from the side tab's context and
a warning on unknown names (secrets offered only as `$NAME`), `⌘S` and the dirty dot, "Send to
pane" (`⌘⇧↵`, bracketed paste into a terminal pane, no Enter), pasted images saved next to the
file (an Untitled buffer asks on save), and a pane status bar with language, encoding, line
endings, line and column. Later: Markdown live preview, tables, outline, a `/` menu, diff
against disk or prompt versions, vim mode, shared editing over the remote view. Rejected:
Monaco (about 5 MB, built for code), Tiptap and Lexical (they rewrite Markdown on save and
produce noisy diffs).

### D30. Files Claude edits
The `PostToolUse` hook for Write, Edit, MultiEdit and NotebookEdit adds each file path to a
Files section of the side tab's context: path relative to the side tab's cwd, added or
modified, time and origin tab. One click opens the file in a text tab. It is a list of
touched files, not a file tree, and it follows the tab rule of D5 when a tab moves.

### D31. Scrollback is a byte ring with resize markers
The daemon keeps the last 8 MB of raw output per pane, capped at 10,000 lines, and records
each resize in the ring. Reattach replays at the recorded width, then resizes to the new one
(D20). Spike 0.1 decides before task 2.1 whether raw replay holds for `claude`; if it does not,
the daemon keeps a headless screen per pane (`alacritty_terminal`) and replays a serialized
screen plus scrollback. Rejected: a line-count ring of decoded text, because it loses colors and
cursor state.

### D32. The daemon runs detached from its own copy
The app copies `terminescu-ptyd` from the bundle to
`~/Library/Application Support/Terminescu/bin/ptyd-<version>` and starts it with
`std::process::Command` and `setsid`. Its socket sits in a `0700` folder with mode `0600`.
The first message on every connection is a handshake with the protocol version; the app speaks
the daemon's version and the one before. A newer daemon takes over only when no pane is open or
when the person picks "Restart sessions", because live ptys cannot move between processes in
v1. Rejected: the Tauri shell plugin's sidecar API, which kills its children when the app exits,
and running the daemon from inside the bundle, which the updater replaces under it.

### D33. Pty output streams as raw bytes; WebGL only where it shows
The core forwards pty output to each window over a Tauri `ipc::Channel` as raw bytes, batched
per pane to one message per frame and at least 1 KB when output is heavy, never as JSON events.
Each window acknowledges what xterm.js has written; past 1 MB unacknowledged the core stops
forwarding and the window resyncs from the ring. WebKit allows 16 live WebGL contexts per web
content process and drops the oldest past that, so only visible panes get the WebGL renderer;
hidden panes and any pane past 12 in a window use the DOM renderer, and `onContextLoss`
falls back to DOM. xterm.js 6 has no canvas renderer. Rejected: Tauri events for output, which
are JSON strings and not built for throughput.

### D34. Hook events map to states, and the hook never gets in Claude's way
`SessionStart` maps the session to the pane. `UserPromptSubmit` sets Running.
`PermissionRequest`, and `Notification` with `notification_type` `permission_prompt` or
`elicitation_dialog`, set Waiting (amber). `PostToolUse`, `PostToolUseFailure` and
`PermissionDenied` clear Waiting back to Running; there is no "approved" event, and
`PreToolUse` fires before the prompt, so it cannot clear it. `idle_prompt` leaves the tab Done:
a finished session is not a request. `Stop` sets Done and carries `last_assistant_message`.
`SessionEnd` drops the agent icon. The main flow is drawn in `sequence.mmd` (rendered to `sequence.svg`). Hooks in `~/.claude/settings.json` run in every Claude
session on the Mac, so `terminescu hook` exits 0 at once when `TERMINESCU_PANE_ID` is unset. It
always exits 0, prints nothing to stdout (stdout from `SessionStart` and `UserPromptSubmit`
reaches Claude's context), gives up on the socket after 100 ms, and is registered with a 5 s
timeout. Rejected: blocking or deciding permissions through hooks, because the app only
watches.

### D35. Variables reach commands without crossing the screen
`terminescu var env` prints `export` lines for the session and secret variables of the calling
pane's side tab. The shell integration runs `eval "$(terminescu var env)"` in `precmd`, so a
shell sees new values at its next prompt, and nothing is typed, echoed, kept in history or
written to the scrollback ring. For Claude, the `SessionStart` hook writes the same `eval` line
to `CLAUDE_ENV_FILE`, which Claude Code runs before each Bash command, so Claude's commands
see `$API_TOKEN` added after Claude started. The limit is stated plainly: any process running as
the person, Claude's Bash tool included, can run `terminescu var get --reveal`. Secret mode
guards against accidental exposure on disk, screen, scrollback, transcript and pins, not
against code that runs as the person. Rejected: typing ` export NAME=value` into the pane,
which echoes the value into the scrollback, needs `HIST_IGNORE_SPACE`, and would land in
Claude's input when Claude is the foreground process.

## Risks

- HTML5 drag and drop does not cross webviews. Drags run on pointer events and the core tracks
  the cursor (D20). WKWebView may stop sending pointer events once the cursor leaves the window;
  the fallback is the core polling the cursor position and mouse button state.
- Raw scrollback replay may garble a TUI mid-redraw; the resize nudge and the
  `alacritty_terminal` fallback cover it (D20).
- An app upgrade must never restart the daemon; its protocol is versioned and stays
  backward compatible (D32). A daemon fix reaches running sessions only after "Restart
  sessions" or a reboot.
- Menu accelerators win over the webview, so a chord bound in the menu never reaches the
  terminal. That is the intent (D17), and Settings › Keys shows which chords the app takes.
- `precmd` runs `terminescu var env` before each prompt; spike 0.4 measures it and the
  integration skips the call when the daemon's variable generation has not changed.
- Claude Code may change hook fields or tool names (`MultiEdit`, `NotebookEdit`); spike 0.5
  records real payloads as fixtures, and the core ignores unknown fields.
- Shell integration varies per shell and breaks under custom prompts; panes fall back to the
  process name and the Idle status.
- Hook payloads change between Claude Code versions; the bridge forwards raw JSON and the
  core parses defensively, with a minimum version check.
- Hot exit writes unsaved text to SQLite; a secret typed into a normal text tab lands there.
  Session-only documents skip hot exit, and the editor offers to mark a buffer session only
  when it holds a secret variable's value.
- A text tab and Claude edit the same file; the disk watcher and the changed-on-disk bar keep
  the person's buffer from overwriting Claude's write silently.
- Transcript JSONL lags the screen; `Stop` carries `last_assistant_message`, used for status,
  while entries come from the file.

## Migration

None; nothing is committed. The Hono, DBOS and Next skeleton is replaced in task 1.1.

## Plan page

`openspec/changes/add-terminescu-foundation/plan.html`, built 2026-11-04. Artifact: https://claude.ai/artifact/NxQVoq2agT798FUijmKxmQ (version 1, 2026-11-04).
