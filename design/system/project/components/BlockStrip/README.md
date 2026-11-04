# BlockStrip

A side tab's tabs as a row of blocks, one per tab, left to right in tab order. It shows on group rows in the sidebar, in palette results and in the MergeDialog.

- Cell states: `faint` idle shell, `blue` running (blinks in two steps), `amber` a Claude session waiting on you, `green` last command or turn done, `red` failed.
- Sizes: `lg` (18x22) in dialogs, default (10x14) in sidebar rows.
- Give the strip an `aria-label` that counts tabs by state ("3 tabs: 1 waiting, 1 running, 1 idle"). Hover names the tab.
- Classes: `.ao-strip`, `.ao-strip.lg`, `.ao-cell` plus `done run wait fail`.
