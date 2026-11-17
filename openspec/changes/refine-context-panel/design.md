## Context

`packages/ui/src/ContextPanel.tsx` draws three `h3` sections, Context, Pins and Prompts.
Context holds three sub-groups (`.ctx-group`): Variables, Links and text, Files. Heads differ:
Variables has a count and "+ variable"; Links has a count and an add form always open under it;
Files has a count; Pins has none; Prompts puts "n saved" and "library ⌘…" on a second line.
Empty sections print a sentence (`.ctx-empty`), and so do the plugin sections (`git.changes`
"No changes.", `claude.todos` "No todo list yet.", `claude.changes` "No changes yet.").
A prompt row's left click calls `onOpenPrompt(key)`, which opens the library sheet on it; its
tooltip is the browser `title`. `renderPrompt` (`insert.ts`) fills `{{name}}` and lists the
names it could not fill. `InsertDialog`'s `PromptStep` asks for those names; Enter there moves
to the next field and `⌘↵` inserts. A document `contextmenu` listener in `App.tsx` opens the
pixel Edit menu unless the event is already `defaultPrevented`.

## Goals / Non-Goals

Goals: one head row per section, no empty-state text, one-line variables, prompt click inserts,
prompt tooltip with blinking values. Non-goals: new add actions for Files or Pins, Enter after
insert, library sheet changes beyond "open on a new prompt".

## Decisions

### D1. Five peer sections, one head component
`SectionHead({ title, count, onAdd, addLabel, adding })` renders
`div.ctx-sec-head > h3 + span.ctx-count + span.grow + button.ctx-add-btn "+"`. It stays sticky
as today. Sections, in order: Variables, Links and text, Files, Pins, Prompts, then plugin
sections with the same head. `data-section` keeps the title, so `Variables`, `Links and text`,
`Files`, `Pins`, `Prompts`. The umbrella "Context" heading goes. Rejected: keeping Context with
sub-heads, which keeps two head styles, the thing item 6 removes.

### D2. Empty means head only
A core section with no rows renders its head and nothing else. Plugin bodies return `null` for
"no rows" states (no session, no shell, no changes, no todos). Real errors still render as
`.ctx-empty` text: an error is news, not a placeholder. Rejected: a dimmed "0" row, which is
still a placeholder.

### D3. Plugin sections report a count
`PanelSectionProps.setCount?(n: number | undefined)`, stable like `setHidden`. The app keeps a
`Map<key, number>` beside `hiddenSections`; the head shows the count once set. Optional, so a
plugin that never calls it shows a head without a count. Additive API change: plugin-api minor
bump in its `package.json` only; no protocol change.

### D4. One-line variables, details in a tooltip
`.ctx-var` becomes one grid row: `name | value | acts`. The acts (mode select, insert, edit, ×)
show on row hover and `:focus-within`, laid over the value's right end on the panel colour, so
the row never grows. `.ctx-meta` leaves the row. Hovering the name or the value shows the pixel
`Tip` with lines: the value (masked for a secret, never the secret), the mode line from
`metaOf`, and the source. A plugin-provided variable gets the same row with its read-only line.
Click on the value still edits in place. Rejected: a native `title`, which can't show the
masked value or follow the TTL tick.

### D5. Links and text add behind `+`
`AddItem` renders only while `adding`, opened by the head `+`, closed on submit or Escape, as
`AddVariable` does.

### D6. Prompt clicks
Left click calls a new `onUsePrompt(prompt)`. `App.tsx`: `renderPrompt` with the shown
context; no `missing` gives `insertChosen(text, false, prompt)`; any `missing` opens
`InsertDialog` with `initialPrompt`, which starts on `PromptStep`; opened that way, Escape and
"‹ all" close the dialog instead of going back to the list. No focused terminal pane gives
`insertChosen`'s existing "Focus a terminal pane to insert." error. Right click calls
`preventDefault()` and `onOpenPrompt(key)`, so the document listener skips its Edit menu. The
head `+` calls `onNewPrompt()`, opening the library on its new-prompt form (`PromptLibrary`
gets `startNew`). Rejected: a right-click menu with Edit and Insert, more clicks for the same.

### D7. Fill-in keys
In `PromptStep`, Enter in a value field calls `go(false)` (skipped while an IME composes):
empty values stay `{{name}}`. Tab and Shift-Tab in a value field move to the next or previous
value field, wrapping, with `preventDefault`; with one field, Tab keeps its normal order. No
`tabIndex` changes, so the checkboxes and buttons stay reachable by clicking or from the
textarea. The textarea keeps Enter as a newline. `⌘↵` and `⌘⇧↵` keep working everywhere. This is one step for both the
panel and `⌘I`, so both change.

### D8. Prompt tooltip
`prompt-tip.ts` exports `tipSlots(body, variables, pins)`: text runs and slots. A slot holds
the raw `{{ name }}` text, the shown value and a width in `ch`:
- value: resolved by the same rule as `renderPrompt`, which this change splits out as
  `resolveName` in `insert.ts`, so a variable named `pins` still wins over the pins; a secret
  shows `$NAME`; `{{pins}}` shows `N pins`; a missing name has no value and never blinks;
- newlines and tabs in a value show as one space;
- cap = max(raw length, 24); width = max(raw length, min(value length, cap)); a value longer
  than the cap shows its first cap - 1 characters and `…`.
`PromptTip` renders the body in the mono font with `white-space: pre-wrap`; each slot is an
`inline-block` of that width in the placeholder colour (`--magenta`, as `.ctx-name`). Both
states fill the same slots, so line breaks and the tooltip's size never change. One `phase`
flips each 1000 ms while the tooltip shows and some slot has a value; the swap is instant
(motion moves in steps). Under `prefers-reduced-motion: reduce` it shows the values, still.
At most 12 lines show; the rest is clipped under a fade. `Tip` gains an optional `body` node,
so the tooltip keeps its frame and placement. The row drops its `title`.

## Risks / Trade-offs

- **Tab held in the value fields** leaves "save" and the buttons to the mouse, `⌘↵` and `⌘⇧↵`.
  Mitigation: a lone field keeps normal Tab order; the a11y e2e runs on the dialog.
- **The library loses its chord hint** in the panel. It stays in the menu, the palette and
  behind right click and `+`.
- **Blink as distraction.** Mitigation: tooltip only on hover, steps, reduced motion honoured,
  constants (1000 ms, 24 ch, 12 lines) in one place for the owner to tune.
- **Plugin empty states vanish.** "No Claude session in this tab" said why a section is empty.
  The claude sections already hide themselves with no session (`setHidden`), so only git loses
  "This tab has no shell", whose head now shows no count.
- **Hover-only acts** on a variable hide insert and × from a glance. Keyboard focus shows them.
- **Selector churn**: e2e tests key on `data-section="Context"` and `.ctx-meta`; task 4 moves them.

## Migration Plan

UI only. No data change. Revert is a plain revert.

## Plan page

Not needed: one module and its plugins, no data model, one PR.
