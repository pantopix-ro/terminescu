# Tasks: harden-plugin-host

## 1. Closes

- [x] 1.1 Runtime exit sink; desktop wires it to note, run, settle (D1). Verify: core test
      "an exit close goes to the sink"; e2e exit then ⇧⌘T.
- [x] 1.2 `SideTabClosed { data }` filled per plugin at dispatch (D2). Verify: plugin host test.
- [x] 1.3 Return edit closes through `sendClose`. Verify: e2e edit-return.

## 2. Reopen

- [x] 2.1 Closed list in `AppState`; a failed reopen keeps its entry (D3). Verify: clippy;
      core `closed` test for putting an entry back.
- [x] 2.2 A row reopens on its shown tab. Verify: e2e reopen case.

## 3. Plugin host

- [x] 3.1 Cap of 4 calls, `Busy` (D4). Verify: plugin host test with hung calls.
- [x] 3.2 Due timers before events (D5). Verify: plugin host test.

## 4. Views

- [x] 4.1 Doc view tag, migration, `openFile` tag (D6). Verify: documents test
      "a pane view tag survives a restart"; e2e "keeps its view until a plain open" (the mock
      keeps no state across a reload).
- [x] 4.2 Diff layout follows `storage`. Verify: ui test.
- [x] 4.3 Pane-bar slot; Claude plugin draws both bars (D7). Verify: e2e hook-off and resume
      bars; with Claude off, no bar.

## 5. Tooling and gate

- [x] 5.1 plugin-api tests run without cargo; `check:rust` runs the seam check.
- [x] 5.2 Full gate and review.
