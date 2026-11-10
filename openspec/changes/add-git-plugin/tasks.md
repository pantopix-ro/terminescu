# Tasks: add-git-plugin

## 1. Provider

- [x] 1.1 `crates/plugin-git` with the `Vcs` trait and `GitCli` facts and changes from
      porcelain v2 and numstat. Verify: `cargo test -p terminescu-plugin-git` against temp
      repositories (clean, dirty, ahead/behind, detached, rebase stopped, untracked, rename).
- [x] 1.2 `TimedVcs`, `CachedVcs` and `CoalescingVcs`. Verify: unit tests over a fake provider
      (timeout drops, mtime invalidates, concurrent requests fold into one follow-up).
- [x] 1.3 Worktrees: add, list, remove, path rules and slugs. Verify: tests
      `worktree_goes_next_to_the_repo`, `worktree_goes_under_the_folder`,
      `an_existing_path_gets_a_suffix`, `remove_with_changes_needs_force`.

## 2. Plugin wiring

- [x] 2.1 The plugin: triggers with a 300 ms debounce per pane, decorations, status facts,
      variables, calls `changes`, `baseText`, `worktreeAdd`, `worktrees`, `worktreeRemove`.
      Verify: plugin tests with a fake `PluginCx`; `pnpm check:rust`.
- [x] 2.2 `GhCli` forge, the GitHub switch, `gh auth status` detection. Verify: tests over a
      fake `gh` script on PATH (open PR, failing checks, no gh, logged out).
- [ ] 2.3 Register behind cargo feature `plugin-git`. Verify:
      `cargo check -p terminescu-desktop --no-default-features --features plugin-claude`.

## 3. UI

- [ ] 3.1 `packages/plugin-git`: branch and PR chips through tab and row decorators, the
      Changes panel section. Verify: `e2e/git.spec.ts` on the mock host with a fake git plugin
      (chip, ahead mark, conflict tone, changes list).
- [ ] 3.2 Diff tab pane view with `@codemirror/merge` 6.12.3, both modes, save on the working
      side, binary notice. Verify: `e2e/git.spec.ts` "a diff tab edits and saves"; vitest for
      the mode toggle.
- [ ] 3.3 "New row from worktree…", "Open worktrees as rows", the close question for worktree
      rows. Verify: `e2e/git.spec.ts` "a worktree row opens below the selected one" and
      "closing a worktree row with changes asks twice".
- [ ] 3.4 GitHub items: "Open pull request", "Check out pull request as a row…". Verify:
      `e2e/git.spec.ts` with and without `gh`.
- [ ] 3.5 Settings › Git: worktree location, diff mode, GitHub switch. Verify:
      `e2e/settings.spec.ts` "the worktree folder setting persists".

## 4. Gate

- [ ] 4.1 Full gate and the seam check. Verify: `pnpm check:rust`, `pnpm -r test`, all e2e,
      `pnpm check:plugins`; manual: a real repository shows its branch within a second of
      `cd`, and a 4-second `git status` repository never delays typing.
- [ ] 4.2 README feature lines and `docs/plugins.md` git section. Verify:
      `openspec validate add-git-plugin --strict`.
