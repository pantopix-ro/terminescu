# Tasks

## 1. Section heads

- [x] 1.1 `SectionHead` and five peer sections in `ContextPanel.tsx`; umbrella "Context" head
      goes; empty sections show the head only (D1, D2). Verify: e2e `context.spec.ts`
      "empty side tab shows five heads and no sentence".
- [x] 1.2 Links and text add box behind the head `+` (D5). Verify: e2e "add a link from the
      head".
- [x] 1.3 Prompts head: count and `+`; `PromptLibrary` `startNew`; the second line goes (D6).
      Verify: e2e `prompts.spec.ts` "plus opens a new prompt".
- [x] 1.4 `setCount` in `PanelSectionProps`, app map, plugin sections report counts and drop
      empty text; errors stay (D2, D3). Verify: `git.spec.ts` count and empty head;
      `claude.spec.ts` todo count; `pnpm -r test`.

## 2. Variables

- [x] 2.1 One-line `.ctx-var`, hover/focus acts, `Tip` with value, mode line and source; secret
      masked (D4). Verify: e2e `context.spec.ts` "variable details on hover", "secret stays
      hidden"; existing edit-in-place test still passes.

## 3. Prompts

- [x] 3.1 Left click `onUsePrompt`: insert when filled, else `InsertDialog` with
      `initialPrompt`; right click opens the library and stops the Edit menu (D6). Verify: e2e
      "click inserts a filled prompt", "click opens fill-in when a value is missing", "right
      click edits", "Escape closes a panel-opened dialog".
- [x] 3.2 Fill-in keys: Enter inserts with empty values kept, Tab cycles value fields (D7).
      Verify: e2e "Enter inserts with a value left empty", "Tab wraps between values";
      `InsertDialog` unit test if one exists, else e2e only.
- [x] 3.3 Split `resolveName` out of `renderPrompt`; `prompt-tip.ts` `tipSlots` uses it (D8). Verify: vitest `prompt-tip.test.ts`: widths,
      ellipsis at 24, a name longer than 24, secret `$NAME`, pins count, a variable named
      `pins`, missing name, newline in value; existing `insert` tests still pass.
- [x] 3.4 `PromptTip` with `Tip` `body`, 1 s phase, reduced motion, 12-line clip (D8). Verify:
      e2e "prompt tooltip blinks at a fixed size" (box size equal across phases, slot text
      changes); reduced-motion e2e shows values only.

## 4. Specs, tests and gates

- [x] 4.1 Update foundation `workbench-ui` "Collapsible context panel" (section order, its
      "All sections at once" scenario and the e2e that checks it) and
      `prompts` "Insert with variables" (Enter and Tab in the dialog) in
      `add-terminescu-foundation`. Verify: `openspec validate add-terminescu-foundation
      --strict` and `openspec validate refine-context-panel --strict`.
- [x] 4.2 Move e2e selectors off `data-section="Context"` and `.ctx-meta`. Verify: full e2e
      `pnpm --filter @terminescu/desktop e2e` passes (port 4173 free, no CI run in progress).
- [x] 4.3 Gates: `pnpm check:rust`, `pnpm -r test`, full e2e. Verify: each exits 0.
- [ ] 4.4 Owner looks at the tooltip in the dev build and tunes 1000 ms / 24 ch / 12 lines if
      needed. Verify: manual.

## Deviations (apply notes)

- 1.4: `packages/plugin-api/package.json` has no `version` field, so there was no minor to bump;
  the additive `setCount` is the only change there.
- 1.4: a plugin reports 0 once it has an answer with no rows, as the core sections do, and
  `undefined` only before its first answer or in a tab with no shell (review fix).
- 2.1: the tooltip opens from the whole variable row (not only the name and value), because the
  hover-only acts lie over the value's right end. The acts fade in with `opacity`, so they stay
  reachable by keyboard focus.
- 3.2: the app's modal focus trap swallowed Tab, so a value field with siblings carries
  `data-own-tab` and the trap leaves its Tab to the dialog (`packages/ui/src/modal.ts`).
- 3.3: D8 measures the raw `{{name}}` text, so the "30-character name" scenario holds for a
  30-character placeholder (a 26-character name); a 30-character name gives a 34-wide slot.
- Review fixes: a panel prompt click with no focused terminal says "Focus a terminal pane to
  insert." before any dialog opens; the insert dialog is keyed by its starting prompt, so a new
  panel click never reuses the old step.
