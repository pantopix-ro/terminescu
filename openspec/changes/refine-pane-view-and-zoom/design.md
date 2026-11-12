# Design

## Context

- The side-by-side view (tasks 8.71, 8.75) is a `View` in `packages/ui/src/view.ts`: a split
  tree whose leaves are tab ids. Only the window knows it; `saveView` keeps it in localStorage
  per window. `hideRow` already removes a leaf and the palette already offers "Hide from the
  window…".
- The area head (App.tsx, near line 2660) has Maximize and Close pane. Close pane sends
  `ClosePane`; the core drops the tab when its last pane goes, and the row with its last tab.
- `pane_to_tab` (`⌘⇧B`) sends `PaneToTab {paneId, index}`; the core moves the pane, process and
  all, into a new tab at `index` of the same side tab.
- Zoom (task 8.8) is `settings.zoom_percent`, 50-200 in `ZOOM_STEPS`. `main.rs` calls
  `set_zoom` on every webview with it, and the `--chrome` CSS var undoes it for the title
  strip. Terminal font size comes from `terminalFontOptions(settings)`.
- A tab's split tree is `Layout` in `crates/core/src/layout.rs`, stored as `tabs.layout_json`.
  `Layout::Pane` already carries optional `resume`, `transcript` and `doc`.
- `windows` has `frame_json` and no zoom column. The last migration is `0014_document_views.sql`.

## Goals / Non-Goals

**Goals:**
- No click on an area head can stop a process.
- Pane zoom and window zoom live in the core, so they survive restarts and moves.

**Non-Goals:**
- Moving the side-by-side view into the core. It stays per window in the UI.
- Changing the zoom steps or range.

## Decisions

### D1. The area × hides; Close pane leaves the head
The × calls `changeView(v => hideRow(v, tab))`, the same path as the palette's "Hide from the
window…". Close pane stays on `⌘W`, the Shell menu and the pane menu, with its existing
confirmation for a busy shell.
Rejected: keep × as Close pane and add a separate hide icon. Two icons side by side invite the
wrong click, and the destructive one is the one people hit today.

### D2. Show Alone composes existing moves
Show Alone runs in the UI: for each other pane of the focused tab, in reading order, send
`PaneToTab` with `index = focusedTabIndex + 1 + n`; then set the view to the one leaf of the
focused tab. No new core command.
Rejected: a core `ShowAlone` command that splits the tab atomically. It would duplicate
`PaneToTab`'s move logic for a gain only visible if the app crashed between two moves, and a
half-done Show Alone still leaves every pane running.

### D3. `⌘⇧M` keeps its meaning
Maximize stays a temporary per-tab state (`maxByTab`); Show Alone is the persistent one.
Rejected: make `⌘⇧M` itself persistent. People rely on the second press to get the split back.

### D4. Pane zoom is a field on the layout leaf
`Layout::Pane` gains `#[serde(default, skip_serializing_if = "Option::is_none")] zoom:
Option<u16>` (percent; `None` is 100). It lives in `layout_json`, so it needs no migration and
moves with the pane through `PaneToTab`, docking and window moves, which all carry the leaf. A
new core command `SetPaneZoom {paneIds: Id[], zoom: u16 | null}` sets it on several panes in one
message, so a folder zoom is one persist and one broadcast. The core clamps to `ZOOM_STEPS`.
Rejected: a `pane_zoom` table keyed by pane id. A table needs cleanup when panes close and a
join on load, for one small number that belongs to the leaf.
Rejected: keep pane zoom in localStorage. It would not follow a pane to another window and is
lost with site data.

### D5. A pane scales its own fonts
`TerminalPane` multiplies `terminalFontOptions(settings).fontSize` by `zoom / 100`, rounds to
0.5 px, and refits, which sends the new size to ptyd as any resize does. `TextPane` and the
transcript pane set their editor font size the same way. Window zoom still applies on top
through `set_zoom`, so the two multiply.
Rejected: CSS `zoom` on the pane element. xterm.js measures cells in CSS pixels and misplaces
the cursor and selection under a scaled ancestor.

### D6. Window zoom moves to the window row
Migration `0015_window_zoom.sql` adds `windows.zoom_percent INTEGER` (NULL means "the Settings
value"). A new core command `SetWindowZoom {windowId, zoom: u16 | null}` writes it and the
window state carries it. `main.rs` calls `set_zoom` on that window's webview only, and each
webview sets `--chrome` from its own value. `settings.zoom_percent` stays, now meaning the
zoom a new window opens at; existing windows read NULL and so keep today's zoom.
Rejected: keep zoom in `frame_json`. The frame is written on every move and resize from the
host side, and a zoom write would race it.

### D7. Focus picks the target
On a zoom action the UI reads `document.activeElement`:
- inside `.ao-tp[data-pane]`: that pane;
- on a sidebar row (`.ao-st`) or folder header: every pane of every tab in it;
- anything else in the window (title strip, tab bar, `.side-fill`, area head, body): the window.
A menu click does not move focus in the webview, so the View menu acts on the same target.
Area heads and the tab bar get `tabIndex={-1}` so a click focuses them.
Rejected: a scope picker in Settings. It adds a mode the person must remember; focus is
already visible through the orange outline.

### D8. A folder steps from its selected pane
A folder zoom takes the selected tab's focused pane zoom (else the first pane's), steps it,
and sets that value on every pane in the folder. `⌘0` sets all to `null`.
Rejected: step each pane from its own value. Panes would keep their differences forever and
the notice could not name one value.

### D9. Notice text
`Pane <name> · N%`, `Folder <name> · K panes · N%` (a one-tab row says `Row <name>`),
`Window · N%`; at a range end, `… · N% (max)` or `(min)`. Show Alone with nothing to do says
`Already alone`. Names come from `tabName`/`paneName`, so no user text is added.

No ptyd protocol change; `PROTOCOL_VERSION` stays 1. The core/UI protocol gains `SetPaneZoom`
and `SetWindowZoom`, and the `Layout` and window types gain optional fields, which old
`layout_json` reads as absent.

## Risks / Trade-offs

- [A focused sidebar row zooms many panes at once by surprise] → the notice names the folder
  and pane count; `⌘0` on the same focus undoes it.
- [Show Alone in a big split creates many tabs] → they land in reading order next to the
  focused tab, and the existing drag docks any of them back.
- [Font scale rounding makes a pane's columns change on each step] → refit sends the new size
  to ptyd, the same path as a window resize; the shell redraws.
- [Window zoom per webview fails on an old Tauri build] → `set_zoom` per window has existed
  since Tauri 2.0; the app pins 2.12.

## Migration Plan

`0015_window_zoom.sql` adds a nullable column; no backfill. Rollback: an older build never
reads the column, and `Layout` has no `deny_unknown_fields`, so it drops the leaf's `zoom` and
opens every window at the Settings zoom.

## Plan page

`openspec/changes/refine-pane-view-and-zoom/plan.html`, built 2026-11-12. Artifact: https://claude.ai/artifact/YFwsvnwNu73M1T7ov3k7pG (version 1, 2026-11-12).
