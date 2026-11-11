# Tasks: add-configurable-shortcuts

## 1. Bindings

- [x] 1.1 Core `keys`: plugin ids, empty means off, a bad saved entry drops alone (D1-D3).
      Verify: `cargo test -p terminescu-core keys`.
- [x] 1.2 Host `keys.ts`: empty bindings, problems and conflicts skip off chords; menu builder
      leaves an off chord out. Verify: `packages/host` vitest; clippy.
- [x] 1.3 `compose` takes bindings, reserves hidden actions' chords (D4). Verify:
      `packages/plugin-api/test/registry.test.ts`.

## 2. Settings › Keys

- [x] 2.1 Plugin actions by plugin, "Wants …, taken by …", off for every row. Verify:
      `e2e/settings.spec.ts` "plugin chords rebind and turn off".

## 3. Diff layout and hints

- [x] 3.1 `diff_layout` app action; the git action goes; `DiffPane` reads its chord (D5).
      Verify: `e2e/git.spec.ts` layout case; `e2e/claude.spec.ts` "⌥⌘U with git off".
- [x] 3.2 `ChordContext` and `useChord`; hints read it (D6). Verify: e2e "a rebind updates the
      hint".
- [x] 3.3 Docs chord test. Verify: `packages/host/test/docs-chords.test.ts`.

## 4. Gate

- [x] 4.1 Full gate and review. Verify: `pnpm check:rust`, `pnpm -r test`, `pnpm typecheck`,
      `pnpm check:plugins`, clippy, all e2e, `openspec validate add-configurable-shortcuts --strict`.
