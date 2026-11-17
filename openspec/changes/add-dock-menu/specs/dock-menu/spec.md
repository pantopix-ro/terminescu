## ADDED Requirements

### Requirement: Dock menu lists windows and rows
A right click on the Dock icon SHALL list every open Terminescu window in workspace order. Each
window SHALL show a header naming it and the row it shows, followed by its sidebar rows in
sidebar order, with the names the sidebar shows. The row a window shows SHALL carry a check mark.

#### Scenario: Two windows
- **WHEN** window 1 holds rows `api` and `web` with `web` shown, window 2 holds `notes`, and the
  person right-clicks the Dock icon
- **THEN** the menu lists `Window 1 · web`, `api`, `web` (checked), a separator,
  `Window 2 · notes`, `notes` (checked), above the items macOS adds

#### Scenario: Renamed row
- **WHEN** the person renames row `api` to `backend` and right-clicks the Dock icon
- **THEN** the menu shows `backend`

### Requirement: Picking from the Dock menu
Picking a window header SHALL bring that window forward, unminimizing it. Picking a row SHALL
bring its window forward and select that row. A pick whose window or row is gone SHALL do nothing.

#### Scenario: Pick a row in a background window
- **WHEN** window 2 is behind window 1 and the person picks `notes` under `Window 2`
- **THEN** window 2 comes forward with `notes` selected and its pane focused

#### Scenario: Pick a minimized window
- **WHEN** window 1 is minimized and the person picks `Window 1 · web`
- **THEN** window 1 unminimizes and comes forward
