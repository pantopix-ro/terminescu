# Spike 0.2: keys

Run on 2026-11-04 with the spike app: one xterm.js pane running a raw-mode key logger (bytes
written to a file) beside a CodeMirror 6 editor. The nine chords are native menu items with
accelerators (D17); xterm.js passes `⌘` chords up through `attachCustomKeyEventHandler` and runs
with `macOptionIsMeta: true`. Key events were posted to the app's pid with `CGEventPostToPid`,
which needed no Accessibility permission. The table shows the second full run.

**Result: go. Every chord fires its menu item in both pane kinds and never reaches the pty or
the editor. Plain, `⌃` and `⌥` keys reach the pty with the expected bytes.**

## Chords

| Chord | Terminal focused | Editor focused |
|---|---|---|
| `⌘T` | menu `new_tab` | menu `new_tab` |
| `⌘W` | menu `close` | menu `close` |
| `⌘K` | menu `palette` | menu `palette` |
| `⌘D` | menu `split` | menu `split`, selection unchanged |
| `⌘1` | menu `tab1` | menu `tab1` |
| `⌘I` | menu `insert` (5 of 5 repeats) | menu `insert` (5 of 5 repeats) |
| `⌘E` | menu `extract` | menu `extract` |
| `⌥⌘T` | menu `text_tab` | menu `text_tab` |
| `⌘\` | menu `context` | menu `context` |
| `⌘B`, `⌘U` (unbound) | keydown reaches the page, nothing reaches the pty | keydown reaches the page, no edit |

The first run missed one `⌘I` in the editor while the app was still starting under heavy load;
ten later repeats all fired.

## Plain keys

| Key | Bytes the pty received | Editor |
|---|---|---|
| `a` | `61` | inserts `a` |
| Return | `0d` | new line |
| Esc | `1b` | no change |
| `←` | `1b 5b 44` | cursor left |
| `⌃C` `⌃A` `⌃R` `⌃U` `⌃G` | `03` `01` `12` `15` `07` | `⌃A` moves to line start, others no change |
| `⌥B` `⌥F` | `1b 62` `1b 66` (Meta) | types `∫` `ƒ` |
| `⌥←` | `1b 5b 31 3b 33 44` | moves by word |
| `⌥⌫` | `1b 7f` | deletes by word |
| Tab | `09` | **moves focus out of the editor** |

## Findings

- D17 holds: menu accelerators win over the webview, including over CodeMirror's `⌘D`.
- CodeMirror's `basicSetup` leaves Tab unbound, so Tab moves browser focus out of the text pane
  and the next keys go nowhere. The text pane binds Tab to indent (`indentWithTab`) and keeps
  Esc then Tab as the way out, as CodeMirror documents.
- In a text pane `⌥` types macOS option characters (`∫`, `ƒ`), as in any Mac editor. Only the
  terminal treats `⌥` as Meta.
- `CGEventPostToPid` lets end-to-end tests script keys on a Mac.
