## 1. Rust half

- [x] 1.1 Tick from cached facts; stop with no branch (D1)
- [x] 1.2 Publish decorations and facts as a diff per tab (D2)
- [x] 1.3 `facts` event carries the root (D3)
- [x] 1.4 Bare layouts in `main_root` and `repo_name`; `row_worktree` never fails (D5)
- [x] 1.5 Orphan event on `SideTabClosed` data; `worktreeRemoveAt` (D6)
- [x] 1.6 `baseText` takes `from` (D7)
- [x] 1.7 `gh auth status --hostname github.com`; normalised worktree folder; `worktreeWhere` (D8)

## 2. UI

- [x] 2.1 `diffTheme` replaces the global diff rules (D8)
- [x] 2.2 DiffPane live handle: base in place, scroll back on layout switch, `sideBySide` reset (D4)
- [x] 2.3 Diff tab reads only for its root (D3)
- [x] 2.4 Renamed and deleted files in the git Changes section; deleted in the Claude one (D7)
- [x] 2.5 Worktree dialog detail from `worktreeWhere`; orphan notice (D6, D8)

## 3. Checks

- [x] 3.1 Rust tests for 1.1 to 1.7; unit tests for 2.2; e2e for 2.4 and 2.5
- [x] 3.2 `docs/plugins.md`; every gate green
