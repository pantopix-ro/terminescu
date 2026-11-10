# Proposal: harden-plugin-host

## Why

A shell that exits closes its pane inside the runtime (`runtime.rs` `pane_exited`). That close
skips ⇧⌘T and the plugins' close hooks, so the row cannot come back and the git plugin loses
the worktree mark it keeps as row data: the save deletes it before the plugin's worker reads
the close event. `⌘↵` in an edit pane closes with a bare `ClosePane` too. A failed reopen loses
its entry. A plugin call that hangs keeps its thread forever, and nothing caps how many pile
up. A busy plugin's timers wait behind its events. A diff tab turns back into a text tab after
a restart, and a layout change in one window does not reach another. The Claude hook-off bar
and the resume bar still live in the core UI.

## What Changes

- A shell exit closes through the same path as `⌘W`: it lands on the ⇧⌘T list.
- The row-closed event carries the plugin's own row data, read before the save deletes it.
- `⌘↵` (return edit) closes through the app's close path.
- Reopen: a row comes back on the tab it showed; a failed reopen keeps its entry and says
  why; the closed list moves from a global into the app state.
- A plugin holds at most 4 calls at once; a fifth is refused as busy.
- An overdue timer runs before the plugin's queued events.
- A document keeps its pane view tag (diff tab) across restarts; a plain open clears it.
- The diff layout follows the `storage` event, so every window shows the same layout.
- The hook-off bar and the resume bar move into a plugin pane-bar slot, drawn by the Claude
  plugin.
- `@terminescu/plugin-api` tests run without cargo; the seam check joins `check:rust`.

Out of scope: text panes coming back with ⇧⌘T (they need a new closed-entry kind and doc
plumbing), hook events lost while the daemon restarts, closing a window.

## Capabilities

### New Capabilities

- `plugin-host`: shell-exit closes, row data on close, call cap, timer order, pane bars,
  persistent pane view tags.

## Impact

- `crates/core`: runtime exit sink, `CoreEvent::SideTabClosed` data, plugin host cap and timer
  order, document view tag (migration).
- `crates/plugin-api`: `SideTabClosed { data }`, `PluginError::Busy`.
- `apps/desktop`: reopen state, exit sink wiring, pane-bar slot, return edit, doc tag.
- `packages/plugin-api`, `packages/plugin-claude`, `packages/ui`, `packages/host` (mock).
