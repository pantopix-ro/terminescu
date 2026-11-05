# Add Terminescu foundation

## Why

A working day runs 5 to 10 Claude Code sessions next to dev servers, test watchers and plain
shells, spread over iTerm windows and tabs with nothing tying them to the ticket they serve.
Prompts live in Sublime or Obsidian and get pasted by hand. The facts worth keeping from a
session (PR URLs, artifact links, branch names, decisions) scroll away. When a session waits
on a permission prompt, nothing says which of the 20 terminals it is in.

The product is a terminal on steroids for Claude: a macOS terminal manager that groups
terminals per ticket, keeps notes per group, and knows when a Claude session needs you. It
replaces the orchestrator this change first proposed (flows, runs, breakpoints, confidence
gates). The product is named Terminescu.

## What Changes

- `Terminescu.app`, a Tauri 2 desktop app for macOS: Rust core as the single source of truth,
  one React 19 and xterm.js webview per window. A Rust pty daemon `terminescu-ptyd` owns every
  shell, so sessions survive closed windows, app restarts and app upgrades. A `terminescu` CLI
  serves as the Claude Code hook and sets statuses and variables from any shell. SQLite holds
  layout, contexts, prompts, commands and statuses, with an `owner_id` on every row.
- Windows, side tabs, tabs, panes. A side tab is a terminal window living in the sidebar: one
  tab shows as a single row, two or more as an expandable group. Its tabs run across the top
  like browser tabs, and each tab splits into terminal, transcript or text panes.
- Text tabs: Terminescu gains the abilities of a text editor, because prompts, notes and commit
  messages get edited by hand next to the terminals. CodeMirror 6 keeps files byte-exact; `⌥⌘T`
  opens an Untitled tab, `⌘O` a file, `⌘`-click a path in terminal output, `terminescu edit`
  from a shell, and an opt-in `EDITOR` hands git and Claude Code's editor requests to a text pane.
  Saving works like any editor, unsaved work survives restarts, and a Files section lists the
  files Claude edited. No file tree.
- Drag and drop like a browser: reorder tabs, drop a tab in the sidebar to make a side tab,
  onto a side tab to move it there, outside the window to open a new window. Drop a side tab
  onto another to merge them, contexts included.
- A context per side tab: variables, pins, links and text shared by all its tabs. `⌘E`
  extracts a selection to a variable, `⌘⇧P` pins a whole message or any selected text,
  extraction rules capture GitHub PR URLs, claude.ai artifact URLs and branch names on their
  own. Variables are saved, session only (never on disk) or secret (masked, inserted as `$NAME`).
- Commands: saved shell lines with `{{variables}}` in a sidebar section, per side tab or global,
  and launcher side tabs that run their commands when opened and after a reboot.
- A prompt library with versions and `{{variables}}`. `⌘I` inserts a prompt or a variable at
  the cursor, resolving variables from the side tab's context and asking for missing ones.
- Claude awareness without wrapping Claude: new tabs start the login shell, and Claude Code
  hooks report which pane a session runs in and whether it waits on you. A minimal transcript
  pane renders the session's messages beside the terminal for pinning and extraction.
- Statuses per side tab and per tab: built-in ones from hooks and shell integration, global ones
  in Settings, local ones saved on a side tab or tab, and dynamic ones set by rules, by
  `terminescu status` or with an expiry. Each has an accent hue and a background tint.
- A Settings screen for statuses, commands, variable defaults, extraction rules, shell and keys,
  with every shortcut editable.
- A context panel that collapses to an auto-hide edge, and panels that scroll on their own.
- The visual system chosen on 2026-11-03 (ANSI ink, Geist Mono, Silkscreen trim, amber for
  "needs you"), and a keyboard model where plain keys always reach the terminal and app
  actions are `⌘` chords.

Dropped from this change, each a possible later change: flows and DAGs, runs, breakpoints and
confidence gates (`add-flows`), per-run worktree workspaces (`add-workspaces`), run secrets,
the MCP `pin` tool (`add-mcp-pin-tool`), the remote view of the Mac in a browser
(`add-remote-view`), other agents such as Codex or OpenCode, and the rich editor layer (Markdown live preview,
tables, outline, `/` menu, diffs, vim mode). DBOS, PostgreSQL, Hono and Next.js leave the
stack.

## Delivery

Three phases on `develop`, each one or more PRs:

1. P1, daily use, after five spikes on reattach, keys, daemon lifetime, throughput and hook
   order, in four milestones that each ship on their own: M1 terminal manager, M2 context and
   prompts, M3 commands, settings and keys, M4 text tabs. Scope: Tauri skeleton replacing the Hono, DBOS and Next skeleton; pty daemon with
   the session store; `terminescu` CLI; side tabs, tabs and splits with persistence and reboot
   recovery; shell integration; Claude status from hooks; minimal transcript pane; prompt
   library and insert; context variables with storage modes and pins; statuses and rules;
   commands and launchers; Settings with editable keys; collapsible context panel;
   notifications; text tabs with openers, editor requests, hot exit and the Files section.
2. P2, the browser moves: drag a tab out to a new window, cross-window drag, side tab merge,
   extraction rules, full transcript pane, send to another tab.
3. P3, reach: code signing and notarization, auto-update. The remote view follows as
   `add-remote-view`.

## Capabilities

- `windows-and-tabs`: windows, side tabs, tabs, panes, drag, merge, detach, layout persistence.
- `terminal-sessions`: the pty daemon, shell integration, reattach with scrollback.
- `claude-awareness`: hook bridge, session-to-pane mapping, transcript mirror.
- `context`: variables, pins, links, extraction, merge rules.
- `prompts`: library, versions, insert with variables, save as prompt.
- `statuses`: built-in, global, local and dynamic statuses, rules, colors, sidebar sort.
- `commands`: saved commands, launcher side tabs.
- `text-editor`: text tabs, openers, editor requests, saving, documents, disk changes.
- `workbench-ui`: visual tokens, keyboard model, attention signals, Settings, panel layout.

## Impact

The uncommitted skeleton (`apps/api`, `apps/web`, Drizzle schema for Postgres) is replaced.
`seeds/templates/*.md` become seed prompts; `seeds/flows/` leaves. Requires Rust stable,
Node 24, pnpm and a logged-in `claude` on the host. tmux is no longer needed.

## Resolved on 2026-11-04

- Name: Terminescu. Directory and change id renamed; no remote yet.
- Tab move: items a tab created move with it; hand-typed items stay.
- Owner column: now, one local owner in v1.
- Transcript pane: minimal in P1.
- Web: a remote view of the Mac, no cloud.
- Text tabs in P1, basic editor first, rich layer later; app shortcuts win in text panes and
  every shortcut is editable; text files save where the person picks; unsaved work survives
  restarts; files Claude edits are listed in the context.

- Plan review: hook event map, secrets through `precmd` and `CLAUDE_ENV_FILE` instead of typed
  exports, detached daemon outside the bundle, raw-byte output channel and WebGL budget, spikes
  first (D31 to D35, `docs/review-2026-11-04.md`).

## Unresolved questions

- tmux-backed sessions as an option for people who want sessions shared with a plain terminal: later or never?
- `⌘K` opens the palette, while Terminal.app and iTerm use it to clear the screen. Keep it, or
  move the palette to `⌘P` and keep `⌘K` for clear?
- A side tab holds one or more tabs, so the core closes it when its last tab closes or moves
  out (task 1.3). Its context (variables, pins, links) goes with it. Keep the context for an
  undo, offer "Close side tab" with a confirmation when the context is not empty, or both?
