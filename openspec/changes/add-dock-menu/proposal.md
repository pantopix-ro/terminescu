## Why

With two or more windows open, a right click on the Dock icon lists none of them, so the owner
cannot jump to a window or a sidebar row from the Dock. A left click spreads the windows out
(task 8.92, PR #23), but a right click is the place macOS users look for a window list.

## What Changes

- The Dock icon's right-click menu lists every Terminescu window, each with its sidebar rows
  (side tabs) under it, in sidebar order.
- Picking a window brings it forward. Picking a row brings its window forward and selects the row.
- The row a window shows now carries a check mark.
- Names match the sidebar exactly: the UI reports each window's row names to the core.
- The menu sits above the items macOS adds itself (Show All Windows, Hide, Quit). With one
  window, the list still shows, so the rows are one click away.

## Capabilities

### New Capabilities
- `dock-menu`: the Dock icon's right-click menu of windows and their rows.

### Modified Capabilities
None.

## Impact

- `apps/desktop/src-tauri/src/main.rs` (or a new `dock_menu.rs`): the menu model, the
  `applicationDockMenu:` method added to the app delegate at setup, `dock:` ids in `send_action`.
- `apps/desktop/src-tauri/Cargo.toml`: `muda = "=0.20.0"` as a direct dependency. It is already in
  `Cargo.lock` through Tauri, so no new library enters the tree. objc2 gains no new features.
- `packages/host` (ipc + mock): `setDockRows`, `onRevealSide`.
- `apps/desktop/src/App.tsx`: push row names on change; select a row on `host://reveal-side`.
- Stacks on `fix/sidebar-folders-windows` (#23), which holds the Dock click code. It rebases
  onto develop after #23 merges.
