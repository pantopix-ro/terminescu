# Tasks

## 1. Hide an area (D1)

- [ ] 1.1 The area head × hides the area through `hideRow`, labelled "Hide from view"; Close
      pane leaves the head. Selection moves to the area that took the space. Verify:
      `split-view.spec.ts` new tests "× hides an area, its shell keeps running" (the pane id
      stays in the workspace, no confirm dialog) and "hiding down to one drops the area heads".
- [ ] 1.2 A hidden view survives a reload and a hidden tab shows again when picked. Verify:
      `split-view.spec.ts` "a hidden tab comes back when picked", with a `page.reload()`.
- [ ] 1.3 Close pane on `⌘W` still confirms a busy shell. Verify: existing close-confirm e2e
      passes unchanged.

## 2. Show Alone (D2, D3)

- [ ] 2.1 `show_alone` action in `packages/host/src/actions.json` (`Cmd+Alt+M`, Shell menu),
      on the area head, in the palette and the pane right-click menu. Verify: `keys.test.ts`
      has no conflict for `Cmd+Alt+M`; `show-alone.spec.ts` "the menu, palette and head all
      offer Show Alone".
- [ ] 2.2 Show Alone moves the focused tab's other panes to new tabs after it via `PaneToTab`
      in reading order and sets the view to the focused tab alone. Verify: `show-alone.spec.ts`
      "split tab beside another area" (pane ids unchanged, `web` hidden, `tests` a new tab at
      index+1) and "one-tab row becomes a folder of three".
- [ ] 2.3 Nothing to do shows "Already alone"; `⌘⇧M` still toggles. Verify:
      `show-alone.spec.ts` "already alone" and the existing maximize e2e.

## 3. Pane zoom (D4, D5, D8)

- [ ] 3.1 `Layout::Pane.zoom: Option<u16>` and `Command::SetPaneZoom {pane_ids, zoom}`,
      clamped to `ZOOM_STEPS`, persisted in `layout_json`, kept by `PaneToTab` and docking.
      Verify: core tests `set_pane_zoom_clamps_and_persists` and
      `pane_zoom_follows_pane_to_tab`; `cargo test -p terminescu-core </dev/null`.
- [ ] 3.2 Host protocol type and mock host support for `SetPaneZoom` and `Layout` pane `zoom`.
      Verify: `pnpm -r test </dev/null` (host protocol tests).
- [ ] 3.3 `TerminalPane`, `TextPane` and the transcript pane scale their font by the leaf's
      zoom and refit. Verify: `zoom-scope.spec.ts` "zoom one pane" (xterm font size 13 → 16.25
      after two steps, the other pane stays 13).
- [ ] 3.4 Folder and row zoom step from the selected pane and set one value on all. Verify:
      `zoom-scope.spec.ts` "zoom a folder" and "folder panes line up".

## 4. Window zoom (D6)

- [ ] 4.1 Migration `0015_window_zoom.sql` adds `windows.zoom_percent`; `SetWindowZoom`
      command; window state carries it. Verify: core test `window_zoom_persists_per_window`
      and `db` migration test over 0015.
- [ ] 4.2 `main.rs` applies each window's zoom (else Settings) to that webview only; each
      webview sets `--chrome` from its own value. Verify: manual check with two windows at
      125% and 90% across a restart; note the result in the PR.
- [ ] 4.3 `settings.zoom_percent` becomes "new windows open at"; Settings copy says so.
      Verify: `crates/core/tests/settings.rs` unchanged and green; SettingsSheet label test.

## 5. Focus scope and notice (D7, D9)

- [ ] 5.1 Rewrite the foundation `workbench-ui` "Zoom" requirement and update task 8.8's text
      in `add-terminescu-foundation` to point at the `zoom` capability. Verify:
      `openspec validate add-terminescu-foundation --strict` and
      `openspec validate refine-pane-view-and-zoom --strict`.
- [ ] 5.2 Zoom actions resolve the target from the focused element (pane, row or folder,
      window); area heads and the tab bar take focus on click. Verify: `zoom-scope.spec.ts`
      "zoom a window" (two windows in the mock host; only the focused one changes) and "plain
      + reaches the terminal".
- [ ] 5.3 The notice names the target and value, with `(max)`/`(min)` at the ends. Verify:
      `zoom-scope.spec.ts` asserts each notice text; the task 8.8 zoom e2e updated.
- [ ] 5.4 Gates: `pnpm check:rust`, `pnpm -r test`, full e2e suite, run after CI is idle so
      port 4173 is free. Verify: the summary lines of each run in the PR body.

## Workflow follow-up

- `/opsx:verify` and `/code-review`, then a PR from `feature/pane-view-zoom` to `develop`.
- Archive after the foundation change archives, since task 5.1 edits its spec.
