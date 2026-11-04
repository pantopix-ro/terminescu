# ContextRow

One item a side tab keeps: a variable, a link or a block of text. All tabs in the side tab share the list. It renders in the context panel on the right of the window.

- Key: the variable name as an `.ao-var` chip. Kind at the right (`text`, `link`, `path`, `json`).
- Value on one line, truncated, links in `link`. Source on the last line: pasted, typed, extracted (with the tab and time), or auto (from an extraction rule such as "GitHub PR URL").
- Click copies the value. `⌘I` in any pane inserts it at the cursor. In a prompt template, `{{name}}` resolves from this list.
- Storage: saved by default. `.session` (dashed outline, `Session` tag) lives in memory only and never reaches disk; it survives an app restart and ends on reboot or when its side tab closes. `.secret` (red dashed outline, `Secret` tag) is session only, masked, left out of pins, copies, merges and exports, and can expire. `⌘I` on a secret types ` export API_TOKEN=…` into the shell and inserts `$API_TOKEN`, so Claude never sees the value.
- A row with `.conflict` (amber edge) only appears in the MergeDialog.
- When a tab moves to another side tab, items it created move with it. Items typed or pasted by hand stay.
