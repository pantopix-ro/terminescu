# Spec Delta

## Purpose
What a window's split view shows, how an area leaves the view without closing its tab, and how
one pane takes the whole window while every other pane keeps running.

## ADDED Requirements

### Requirement: Hide an area from the view
When a window shows two or more tabs side by side, each area head SHALL carry a × labelled
"Hide from view" that hides that area. Hiding SHALL NOT close a pane, stop a process or remove
the tab from the sidebar. The remaining areas SHALL take the freed space in proportion to their
shares, and a selection in the hidden area SHALL move to the area that took its place.

#### Scenario: Hide one of three areas
- **WHEN** a window shows tabs `api`, `web` and `logs` side by side and the person clicks × on `logs`
- **THEN** `api` and `web` fill the window, `logs` stays in the sidebar with its shell running, and no confirmation asks

### Requirement: A hidden tab shows again
A hidden tab SHALL show again when the person picks it in the sidebar or the tab bar, or docks
it beside a pane. The view, hidden areas included, SHALL survive a restart.

#### Scenario: Hide down to one
- **WHEN** a window shows `api` and `web` and the person clicks × on `web`
- **THEN** `api` fills the window with no area heads, and picking `web` in the sidebar shows `web` in place of `api`

#### Scenario: Hidden view survives a restart
- **WHEN** the person hides `logs` and restarts the app
- **THEN** the window opens with `api` and `web` side by side and `logs` in the sidebar

### Requirement: Close pane keeps its own entry points
Close pane SHALL stay on `⌘W`, the Shell menu and the pane's right-click menu, and SHALL NOT
be on the area head. It SHALL ask before closing a pane whose shell runs a command, as it does
today.

#### Scenario: ⌘W on a busy pane
- **WHEN** the focused pane runs `pnpm dev` and the person presses `⌘W`
- **THEN** a confirmation names the running command, and the pane closes only after it is confirmed

### Requirement: Show a pane alone
The action Show Alone (`⌥⌘M`, the area head, the palette and the pane's right-click menu) SHALL
leave the focused pane as the only thing in the window. Every other area SHALL leave the view
as "Hide from view" does. Every other pane of the focused tab SHALL move to its own new tab
right after the focused tab, in the same row or folder, in reading order. No pane SHALL close
and no process SHALL stop.

#### Scenario: Show alone from a split tab beside another area
- **WHEN** the window shows tab `api` (panes `server` and `tests`) beside tab `web`, `server` has focus, and the person presses `⌥⌘M`
- **THEN** `server` fills the window, `tests` is a new tab right after `api` in the same folder with its shell still running, and `web` is hidden but still in the sidebar

### Requirement: Show Alone edge cases
A one-tab row whose tab splits through Show Alone SHALL become a folder of those tabs. When the
focused pane is already alone in the window, the action SHALL change nothing and the notice
SHALL say so. Show Alone SHALL persist, while `⌘⇧M` SHALL stay a temporary maximize that a
second press undoes.

#### Scenario: Show alone in a one-tab row
- **WHEN** a one-tab row holds a tab with panes `a`, `b` and `c`, `a` has focus, and the person runs Show Alone from the palette
- **THEN** the row becomes a folder of three tabs, the first showing `a` alone and selected

#### Scenario: Nothing to do
- **WHEN** the focused pane already fills the window and the person presses `⌥⌘M`
- **THEN** nothing changes and the notice reads "Already alone"

#### Scenario: Maximize stays temporary
- **WHEN** a tab has two panes and the person presses `⌘⇧M` twice
- **THEN** the focused pane fills the tab and then both panes show again, with no new tab
