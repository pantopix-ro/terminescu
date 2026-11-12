# Spec Delta

## Purpose
Zoom scoped by what has focus: one pane, every pane of a sidebar row or folder, or one window,
with its range, steps, persistence and the notice that names the target.

## ADDED Requirements

### Requirement: Focus decides the zoom target
`⌘=` and `⌘+` SHALL zoom in one step, `⌘-` SHALL zoom out one step and `⌘0` SHALL go back to
100%. With a pane focused (terminal, text or transcript) they SHALL zoom that pane's text. With
a sidebar row or folder header focused they SHALL zoom every pane of its tabs. With window
chrome focused (title strip, tab bar, empty sidebar space, an area head) they SHALL zoom that
window's whole UI. The View menu SHALL act on the same target. Nothing else SHALL change.

#### Scenario: Zoom one pane
- **WHEN** a tab shows panes `server` and `tests`, `tests` has focus, and the person presses `⌘+` twice
- **THEN** the text of `tests` shows at 125% and refits to its pane, and `server` stays at 100%

#### Scenario: Zoom a folder
- **WHEN** folder `acme` holds two tabs with four panes in all, the folder header has focus, and the person presses `⌘+`
- **THEN** all four panes show at 110%, and panes outside `acme` keep their zoom

#### Scenario: Zoom a window
- **WHEN** two windows are open, the tab bar of the first has focus, and the person presses `⌘+`
- **THEN** the first window's whole UI shows at 110% and the second window stays at 100%

#### Scenario: Plain keys still reach the terminal
- **WHEN** a terminal has focus and the person types `+`
- **THEN** the shell receives `+` and nothing zooms

### Requirement: Zoom range and steps
Pane and window zoom SHALL each run from 50% to 200% in the steps the app already uses. A pane
zoom SHALL apply on top of its window's zoom, so a pane at 110% in a window at 125% shows its
text at about 137%. A folder zoom SHALL step from the zoom of the folder's selected pane and
SHALL set every pane in the folder to that one value.

#### Scenario: Folder panes line up
- **WHEN** a folder holds panes at 90% and 125%, its selected pane is the one at 90%, and the person presses `⌘+` on the folder header
- **THEN** both panes show at 100%

#### Scenario: The range ends
- **WHEN** a pane is at 200% and the person presses `⌘+`
- **THEN** it stays at 200% and the notice says it is at the maximum

### Requirement: Zoom is saved
A pane's zoom SHALL be saved with that pane and SHALL follow it when it moves to another tab,
row or window. A window's zoom SHALL be saved with that window. A new window SHALL open at the
zoom saved in Settings, which starts at the app-wide zoom the person had before this change.

#### Scenario: Pane zoom kept
- **WHEN** the person zooms pane `tests` to 125%, drags it into another tab and restarts the app
- **THEN** `tests` opens at 125% in its new tab

#### Scenario: Window zoom kept
- **WHEN** the person zooms one window to 125% and the other to 90%, then quits and reopens the app
- **THEN** each window opens at its own zoom

### Requirement: The zoom notice names the target
After each zoom step a short notice SHALL name the target and the new value, as
`Pane tests · 125%`, `Folder acme · 4 panes · 110%` or `Window · 125%`.

#### Scenario: Notice for a folder
- **WHEN** the person presses `⌘-` with the header of folder `acme` (4 panes) focused at 110%
- **THEN** the notice reads `Folder acme · 4 panes · 100%`
