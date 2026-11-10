# Proposal: add-configurable-shortcuts

## Why

Settings › Keys rebinds the app's own actions, but a plugin's chords are fixed. A plugin that
loses its chord to the app loses it without a word. The diff layout chord (⌥⌘U) belongs to the
git plugin, so a Claude diff has none when git is off. No app shortcut can be turned off, and
about fifteen hints in the UI print a default chord that a rebind leaves wrong.

## What Changes

- Plugin actions list in Settings › Keys under their plugin, and each can be rebound, reset or
  turned off. The person's chord wins over a plugin's default.
- Any app shortcut can be turned off. An empty binding means no chord.
- A plugin chord the app or another plugin holds shows as "Wants ⌥⌘G, taken by …".
- A hidden plugin action (its `when` flag down) still holds its chord, so nothing takes it
  meanwhile.
- "Toggle Diff Layout" becomes an app action under View, ⌥⌘U by default, for every diff tab.
- UI hints show the chord as bound now, and hide when it is off.
- A test checks that every ⌘ chord the README and `docs/` name is a real default.

Out of scope: dialog-local keys (Esc, ↵, ⌘↵ in Insert, ⌘S in the prompt library), the macOS
chords in `RESERVED`, and CodeMirror's own keymap inside text panes.

## Capabilities

### New Capabilities

- `shortcuts`: plugin chords in Settings › Keys, turning a chord off, live chord hints, the
  diff layout as an app action.

## Impact

- `crates/core/src/keys.rs`: plugin ids and empty bindings; a bad saved entry drops alone.
- `packages/host/src/keys.ts`, `actions.json`: empty bindings, `diff_layout`.
- `packages/plugin-api/src/index.ts` `compose`: person's bindings, hidden actions reserve.
- `packages/ui`: `ChordContext`, Keys page, hints; `DiffPane` reads its chord from the app.
- `packages/plugin-git`: the diff layout action goes.
- `apps/desktop`: menu builder skips empty chords; `act("diff_layout")`.

## Unresolved questions

None.
