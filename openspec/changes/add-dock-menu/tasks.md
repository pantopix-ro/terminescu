## 1. Core menu model

- [x] 1.1 `dock_menu.rs`: `Row { id, name, active }`, `entries(order, rows) -> Vec<Entry>`
      (header, rows, separators) as a pure function. Verify: unit tests for two windows, a
      window with no rows reported yet (header only), stale window dropped.
- [x] 1.2 `dock_menu::Rows` managed state (kept out of `AppState`); `#[tauri::command] dock_rows(window, rows)`; drop entry on
      `WindowEvent::Destroyed`. Verify: `pnpm check:rust`.

## 2. Native menu (macOS)

- [x] 2.1 `muda = "=0.20.0"` direct dep (already locked); build `muda::Menu` from entries,
      keep it in a `thread_local!`. Verify: `cargo tree -d` shows one muda.
- [x] 2.2 In `setup`, `class_addMethod(applicationDockMenu:)` on the delegate class; log when
      it returns false. Verify: manual right click shows the list.
- [x] 2.3 `send_action`: `dock:w:` / `dock:s:` ids focus the window, emit `host://reveal-side`.
      Verify: unit test of id parsing; stale id is a no-op.

## 3. UI

- [x] 3.1 host ipc + mock: `setDockRows`, `onRevealSide`. Verify: `pnpm -r test`.
- [x] 3.2 App.tsx: debounced push of `{ id: sideTabName, active }` per window; on reveal-side
      select the row and refocus. Verify: e2e "a reveal-side event selects the row" and
      "the window reports its rows as the sidebar names them" and "a Dock menu pick selects
      the row" on the mock host (`e2e/dock-menu.spec.ts`).

## 4. Gates

- [x] 4.1 `pnpm check:rust`, `pnpm -r test`, e2e, `openspec validate add-dock-menu --strict`.
- [ ] 4.2 Manual (owner): two windows, right-click the Dock icon, pick a row in the back window;
      minimize one and pick its header.
