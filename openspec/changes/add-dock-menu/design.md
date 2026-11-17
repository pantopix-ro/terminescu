## Context

Tauri 2.12 has no Dock menu API. macOS asks the app delegate's `applicationDockMenu:` for one
each time the person right-clicks the icon. tao declares the delegate class
(`TaoAppDelegateParent`) and does not implement that method. Tauri forwards every muda menu
click to `on_menu_event`, which already routes to `send_action`.

## Goals / Non-Goals

**Goals:** list windows and rows; pick one to show it; names identical to the sidebar.

**Non-Goals:** top-bar tabs inside rows (owner chose rows only); a menu bar (status) icon;
actions such as New Window in the Dock menu.

## Decisions

**D1. Build the menu with muda, hand its NSMenu to macOS.** A `muda::Menu` gives
`ContextMenu::ns_menu()`, and its clicks already reach `on_menu_event`. Rejected: hand-written
NSMenu items with an objc target class (more unsafe code, its own click routing).

**D2. Add `applicationDockMenu:` to the delegate's class at setup.** In `setup`, read
`NSApp.delegate`, take its class, and `class_addMethod` a `extern "C-unwind"` function that
returns the current menu's NSMenu pointer, or nil when there is none. AppKit caches which
delegate methods exist when the delegate is set, so setup then sets the same delegate again.
The function reaches the app through an `AppHandle` kept in a `OnceLock` from setup.
`class_addMethod` fails cleanly if a later tao adds the method; we log it and the Dock shows
macOS's default menu.
Rejected: subclassing or swizzling the delegate (fragile against tao updates).

**D3. The UI reports names; the core orders windows.** Row names come from `sideTabName`, which
uses live terminal names and document titles the core does not hold. Each window calls
`dock_rows(rows: [{ id, name, active }])` when its rows, names or selection change (debounced
250 ms). The core keeps `Mutex<HashMap<window label, Vec<Row>>>` in `AppState`, drops a window's
entry on `WindowEvent::Destroyed`, and orders windows by the workspace snapshot, which also leaves out the hidden drag ghost.
Rejected: naming rows in Rust (would drift from the sidebar).

**D4. Rebuild on demand.** The delegate method runs on the main thread. It builds a fresh
`muda::Menu` from the stored rows and keeps it in a `thread_local!` so the NSMenu stays alive
until the next request. The windows come from the workspace snapshot, filtered to open webviews. Rows number in the tens, so a rebuild costs well under 1 ms.

**D5. Layout.** Per window: an enabled header `Window 1 · <active row name>`, then its rows as
items indented one level (`setIndentationLevel` is not exposed by muda, so rows get a four-space prefix instead), the active row as a `CheckMenuItem` checked. A separator between
windows. A folder row (a side tab holding several tabs) shows its folder name; its tabs are not listed.

**D6. Ids and picking.** Items use ids `dock:w:<window>` and `dock:s:<window>:<side tab>`.
`send_action` handles the `dock:` prefix before action lookup: unminimize and focus the
window (as `attention_jump` does), then for a row emit `host://reveal-side` with the side tab
id to that window. The UI calls `setSideTabId` and refocuses its pane. A stale id (window or
row gone) does nothing.

## Risks / Trade-offs

- [macOS may show both our items and its own window list] → ours come first; acceptable, and
  the owner reported that the default list showed no windows at all.
- [tao starts implementing `applicationDockMenu:`] → `class_addMethod` returns false, logged;
  fall back to default menu. A unit test cannot catch it; the manual check after Tauri bumps does.
- [Names lag a live terminal rename by up to 250 ms] → acceptable for a menu opened by hand.
- [Native menu is not testable in e2e] → the menu model is a pure function with Rust unit tests;
  the UI side (push, reveal) has e2e on the mock host; one manual Dock check.

## Open Questions

None.
