# MergeDialog

Shown when a side tab is dropped onto another side tab. The tabs move, and the two contexts merge into one.

- Subject line: source into target. Summary: tabs that move, items that merge, names that clash.
- Items without a clash merge silently and show a green `new` or `added`.
- A clash is a variable name present in both with different values. It gets an amber edge and two choices: keep the target's value, or keep both and rename the source's (`{{branch_477}}` by default, editable).
- Pins, links and prompts never clash: they are lists and simply concatenate, each keeping its origin tab.
- No clash: the dialog is skipped and a `Merged` toast appears with Undo (`⌘Z`) for 10 s.
