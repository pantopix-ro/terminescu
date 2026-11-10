# Design: add-configurable-shortcuts

## Context

Chords come from `packages/host/src/actions.json` (app) and `PluginAction.accel` (plugins).
The person's changes live in the `keys.bindings` setting, checked in `crates/core/src/keys.rs`
and mirrored in `packages/host/src/keys.ts`. Plugin chords reach the native menu through
`compose` and `plugin_menu`.

## Decisions

### D1. One bindings map, plugin ids included

Plugin bindings sit in `keys.bindings` under the action's full id (`git.newWorktreeRow`, say). The
core checks their form (a key, ⌘, not reserved) and their clash with app chords; it cannot know
plugin defaults, so clashes between plugins resolve in `compose`.
Rejected: a second setting for plugin bindings. Two stores would need two saves and two
reset-all paths for one page.

### D2. Empty means off

A binding of `""` turns the action's chord off, for app and plugin actions alike. The menu
item stays, without an accelerator.
Rejected: a separate "disabled" list. The binding already says which chord; off is one value of it.

### D3. A bad saved entry drops alone

Reading the bindings drops each entry that fails the checks and keeps the rest. Today one bad
entry (an action renamed, a default that moved onto a saved chord) drops every binding.
Saving still rejects the whole change, so the page shows the problem.

### D4. Person first, then plugin defaults in plugin order

`compose` takes the bindings. It reserves the app's chords and `RESERVED`, then the person's
plugin bindings, then plugin defaults in plugin order. A hidden action reserves its chord too,
before the `when` filter, so a flag rising never moves a chord.
Rejected: reserving only shown actions (today). A chord would move between actions as flags
change.

### D5. Diff layout is an app action

`diff_layout` joins actions.json (View, `Cmd+Alt+U`). The app toggles the shared layout
(`toggledDiffMode`), which every `DiffPane` follows. This replaces add-claude-insights' note
that the chord "stays in the git plugin".
Rejected: a chord per plugin. Two plugins would fight over one chord for one setting.

### D6. Hints read the live chord

`ChordContext` in `@terminescu/ui` maps an action id to its chord label, or nothing when off.
Hints that print a chord read it through `useChord(id)`.
Rejected: keep fixed hint strings. They go wrong on the first rebind.

## Risks

- A menu rebuilt with an empty accelerator: Tauri takes `None`; covered by the e2e and the
  menu test.
- Saved bindings of a plugin that is later removed stay in the map; the read drops them only
  when they clash. They do no harm.
