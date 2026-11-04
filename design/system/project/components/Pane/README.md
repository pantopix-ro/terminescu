# Pane

One terminal inside a tab. A tab splits into panes like iTerm or tmux: `⌘D` splits right, `⌘⇧D` splits down, `⌘⌥` plus an arrow moves focus, dragging the 2px divider resizes.

- Kinds: `terminal` (xterm.js on a pty) and `transcript` (the rendered JSONL of a Claude session in a sibling pane, where text is easiest to select, pin and extract).
- Head, 22px: kind icon, process and cwd in `small faint`. The focused pane gets a 2px amber inset and an amber head. Exactly one pane has focus in a window.
- Terminal colors: prompt `green`, tool calls `cyan`, links `blue` underlined, secondary output `dim`. The xterm theme reads the same tokens.
- Dividers are the `line-strong` ground showing through a 2px grid gap. No pane has its own outer border.
- Classes: `.ao-split` (set the grid template inline), `.ao-tp` (+ `.focused`), `.ao-tp-head`, `.ao-tp-body`, `.ao-term`.
