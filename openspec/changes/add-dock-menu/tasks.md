## 1. Core menu model

- [ ] 1.1 `dock_menu.rs`: `Row { id, name, active }`, `entries(order, rows) -> Vec<Entry>`
      (header, rows, separators) as a pure function. Verify: unit tests for two windows, a
      window with no rows reported yet (header only), stale window dropped.
- [ ] 1.2 `AppState.dock_rows`; `#[tauri::command] dock_rows(window, rows)`; drop entry on
      `WindowEvent::Destroyed`. Verify: `pnpm check:rust`.

## 2. Native menu (macOS)

- [ ] 2.1 `muda = "=0.20.0"` direct dep (already locked); build `muda::Menu` from entries,
      keep it in a `thread_local!`. Verify: `cargo tree -d` shows one muda.
- [ ] 2.2 In `setup`, `class_addMethod(applicationDockMenu:)` on the delegate class; log when
      it returns false. Verify: manual right click shows the list.
- [ ] 2.3 `send_action`: `dock:w:` / `dock:s:` ids focus the window, emit `host://reveal-side`.
      Verify: unit test of id parsing; stale id is a no-op.

## 3. UI

- [ ] 3.1 host ipc + mock: `setDockRows`, `onRevealSide`. Verify: `pnpm -r test`.
- [ ] 3.2 App.tsx: debounced push of `{ id: sideTabName, active }` per window; on reveal-side
      select the row and refocus. Verify: e2e "a reveal-side event selects the row" and
      "renaming a row reports the new name" on the mock host.

## 4. Gates

- [ ] 4.1 `pnpm check:rust`, `pnpm -r test`, e2e, `openspec validate add-dock-menu --strict`.
- [ ] 4.2 Manual (owner): two windows, right-click the Dock icon, pick a row in the back window;
      minimize one and pick its header.
