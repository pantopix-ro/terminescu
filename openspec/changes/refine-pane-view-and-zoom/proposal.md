# Proposal

## Why

A split screen has no way back to one pane. The area head's × runs Close pane, which ends the
shell and, with the last pane, the tab and its row; `⌘⇧M` only maximizes until pressed again.
Zoom is one setting for every window, so a person can't enlarge one Claude session on a
projector or shrink a log pane beside it.

## What Changes

- The × on an area head of the side-by-side view (task 8.75) hides that area from the view. The
  tab keeps running and stays in the sidebar; picking it shows it again. Close pane stays on
  `⌘W`, the Shell menu and the pane's right-click menu, and still asks when something runs.
- New action **Show Alone** (`⌥⌘M`, the area head, the palette, the pane's right-click menu):
  the focused pane fills the window. Other areas leave the view. The focused tab's other panes
  each move to their own tab beside it, in the same row or folder, as `⌘⇧B` does. Nothing
  closes and no process stops. `⌘⇧M` stays the temporary maximize.
- **BREAKING** (behaviour) `⌘+`, `⌘-` and `⌘0` zoom what has focus:
  - a pane (terminal, text or transcript) scales its own text; the zoom is saved with the pane;
  - a sidebar row or folder with focus scales every pane of its tabs;
  - window chrome with focus (title strip, tab bar, empty sidebar space, area heads) zooms that
    window's whole UI, saved per window.
  Today every one of these zooms all windows at once.
- The zoom notice names the target: `Pane api · 125%`, `Folder acme · 4 panes · 110%`,
  `Window · 125%`.

## Capabilities

### New Capabilities
- `pane-view`: what a window's split view shows, how an area leaves it without closing its
  tab, and how one pane takes the whole window while the rest keep running.
- `zoom`: zoom scoped by focus (pane, sidebar row or folder, window), its range, steps,
  persistence and notice.

### Modified Capabilities
None in `openspec/specs/`. The foundation change `add-terminescu-foundation` is not archived
yet; its `workbench-ui` "Zoom" requirement (every window zooms at once) conflicts with `zoom`.
Task 5.1 rewrites that requirement in the foundation change so both archive without a
contradiction.

## Impact

- `crates/core`: `Layout::Pane` gains an optional `zoom` (percent) kept in `layout_json`; a
  new command sets it on a set of panes. Migration `0015_window_zoom.sql` adds
  `windows.zoom_percent`. No ptyd protocol change, so `PROTOCOL_VERSION` stays 1.
- `apps/desktop/src-tauri/src/main.rs`: `set_zoom` per window from its own value, not from
  `settings.zoom_percent`, which becomes the zoom a new window starts at.
- `packages/host`: protocol types for the new command and window zoom; actions.json adds
  `show_alone` (`Cmd+Alt+M`).
- `packages/ui`, `apps/desktop/src/App.tsx`: area head ×, Show Alone, focus-scoped zoom,
  per-pane font scale in `TerminalPane`, `TextPane` and the transcript pane.
- e2e: `split-view.spec.ts`, new `show-alone.spec.ts`, new `zoom-scope.spec.ts`; the zoom
  test of task 8.8 changes.

Out of scope: a gesture zoom (pinch, `⌘` scroll), zooming the sidebar alone, and an undo for
Show Alone (the moved panes are tabs and can be docked back with the existing drag). None has
an owning change yet.

## Unresolved questions

1. Show Alone in a one-tab row with three panes turns the row into a folder of three tabs.
   Recommended: accept it, since a folder is how the app shows several tabs of one context.
2. Should a pane with its own zoom ignore window zoom, or multiply with it? Recommended:
   multiply (window 125% × pane 110%), so window zoom still enlarges everything.
3. `⌥⌘M` for Show Alone. Recommended; it sits next to `⌘⇧M` maximize and is free today.
