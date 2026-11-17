## Why

The context panel spends its height on empty-state sentences, a second line under Prompts and
an add box that never closes. Each section draws its head differently. A prompt in the panel
opens the library editor on a left click, though the common act is to put the prompt in the
terminal. A prompt's tooltip shows its raw body, so the person can't see what the placeholders
will become.

## What Changes

- Five peer sections, Variables, Links and text, Files, Pins and Prompts, each with one head
  row: the title, the count and a `+` where the section has an add action. The umbrella
  "Context" heading goes; the panel header already says Context.
- An empty section shows its head row only. No empty-state sentence, in core or plugin
  sections. Errors still show.
- Variables take one line each; the mode, expiry, source and full value move to a pixel
  tooltip.
- Links and text: the add box opens from the head's `+`, as Variables' does.
- Prompts: the count and the library button leave the second line; `+` opens a new prompt in
  the library.
- Prompt left click inserts the filled prompt into the focused pane as a paste, without Enter.
  If a placeholder has no value, the fill-in dialog opens on that prompt. Right click opens the
  prompt in the library editor.
- Fill-in dialog: Enter inserts, also with empty values, which stay `{{name}}`. Tab and
  Shift-Tab move between the value fields only. This applies to the `⌘I` dialog too.
- Prompt tooltip: the full body, its placeholders blinking once a second between `{{name}}` and
  the value, in the placeholder colour, at a fixed size in both states.
- Plugin API: a panel section can report its count to the head (`setCount`).

## Capabilities

### New Capabilities
- `context-panel`: the panel's section heads, compact rows, prompt clicks and the prompt
  tooltip.

### Modified Capabilities
- None in `openspec/specs`. The foundation's `workbench-ui` "Collapsible context panel" and
  `prompts` "Insert with variables" requirements still live in the unarchived
  `add-terminescu-foundation` change; task 5.1 updates them in place.

## Impact

- `packages/ui`: `ContextPanel.tsx`, `InsertDialog.tsx`, `PromptLibrary.tsx`, `Tip.tsx`, a new
  `PromptTip.tsx` and `prompt-tip.ts`.
- `packages/plugin-api`: optional `setCount` on `PanelSectionProps`.
- `packages/plugin-claude`, `packages/plugin-git`: drop empty-state text, report counts.
- `apps/desktop`: `App.tsx` wiring, `app.css`, e2e `context.spec.ts`, `prompts.spec.ts`,
  `git.spec.ts`, `claude.spec.ts`.
- No Rust, no migration, no protocol change.

## Out of scope

- An add action for Files or Pins (owner, 2026-11-17: title and count only).
- Sending Enter after a panel insert (owner: no).
- Any change to the library sheet besides opening on a new prompt.

## Resolved questions (owner, 2026-11-17)

1. Unfilled placeholders on left click: open the fill-in dialog; Enter inserts with empty
   values; Tab moves between values.
2. Enter after paste: no.
3. Drop the umbrella "Context" heading: yes.
4. `+` on Files and Pins: none.
