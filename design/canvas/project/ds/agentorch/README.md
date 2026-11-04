Terminescu is a terminal manager for macOS, built for days spent in Claude Code. Each side tab in the sidebar is a terminal window living inside the app: it holds one or more tabs, each tab splits into panes, and the side tab keeps a context that every tab in it shares. The context holds variables, pins, links and prompts. Text goes into a session from it with `⌘I` and comes out of a session into it with `⌘E` and `⌘⇧P`. The screen answers one question first: which session needs me now.

The look is a dark terminal with pixel trim on top. Panes, text and data are plain Geist Mono on near-black. The pixel layer (Silkscreen labels, notched frames, hard shadows, 8x8 icons, block strips) marks what a person acts on: status labels, keys, dialogs, saves, drag targets.

## Content fundamentals

- Write like a good CLI: short, lowercase where the terminal would be, sentence case for buttons. "New side tab", "Save as prompt", "Merge".
- Lead with the side tab, then the tab, then the fact: "PROJ-482 · claude · plan asks to run a command". Never "Your session needs attention".
- Numbers stay numbers: "3 tabs", "22m", "412 words".
- Address the person as "you" only where a decision waits ("1 needs you"). No exclamation marks, no emoji, no mascot voice.
- Pixel labels are one or two words, uppercase by the face: `SAVED`, `WAITING`, `REVIEW`. Sentences never go in Silkscreen.
- Sample ticket text in mockups is invented (`PROJ-482 · audit export`, `acme/api`). Never paste a client's ticket into a design file.

## Color

One dark theme in v1. The palette is ANSI ink: a near-black ground, two panel steps, six signal hues, and five tints for the person's own sorting.

- Ground `bg`, terminal panes and rows `panel`, heads and inputs `panel-2`. Hairlines `line`, pane dividers and frames `line-strong`.
- Text: `fg` for content, `dim` for metadata, `faint` only for pane heads, disabled glyphs and empty cells.
- `amber` means "needs you": a Claude session waiting on a permission or a question, the focus ring, drop targets while dragging, the primary action. The person can't pick it as an accent, so it never loses that meaning.
- `blue` running and links, `green` done, `red` failed, `magenta` pins and variables, `cyan` tool calls in Claude output.
- People color their own work. A side tab or tab takes an accent (a 4px edge, one of `blue cyan green magenta red fg`) and a background (`tint-blue`, `tint-cyan`, `tint-green`, `tint-magenta`, `tint-red`, each the hue mixed 16% into `panel`). `fg` keeps 11:1 or better on every tint.
- `selection` paints selected text before Pin or Extract.
- A status hue always travels with a glyph or a word. Green and red are never the only difference between two states.

## Type

- `mono` (Geist Mono) carries every readable word. `body` 13/20 is the default; `title` 18/24 for dialog subjects; `small` 12/16 for meta, tab titles and the status line; `term` 13/18 inside xterm.js panes.
- `pixel` (Silkscreen) is trim: `pixel-label` 8/12 on status labels, chips, keycaps and pane heads; `pixel-heading` 16/20 for dialog titles; `pixel-display` 32/32 for the boot screen and the cover only.
- Silkscreen renders at multiples of 8px only. Any other size blurs the pixel grid.

## Space, shape and depth

- 4px base: `space-1` 4, `space-2` 8, `space-3` 12, `space-4` 16, `space-6` 24. Pixel ornaments use `px` (2px).
- Corners are square (`radius-0`). The notched frame replaces the rounded corner (`frame-neutral`). Inputs get `radius-1`, tooltips `radius-2`, nothing else rounds.
- Depth is a hard shadow with no blur: `shadow-hard` (6px) under floating frames and the drag ghost, `shadow-hard-sm` (3px) under chips and buttons. Panes and rows sit flat.
- Focus is `focus-ring`: a 2px ground gap, then 2px solid `amber`.

## Layout

A window has four regions:
- **Sidebar**, 272px, can collapse to a 48px rail: the side tabs (SideTab), each a single row or an open or closed group.
- **TabBar** across the top of the main area: the active side tab's tabs, like browser tabs.
- **Pane area**: the active tab, split into terminal and transcript panes with 2px dividers. Exactly one pane has focus, shown by an amber inset.
- **Context panel**, 304px, on the right: the side tab's variables, pins, links and prompts. `⌘\` switches it between pinned and auto-hide. Auto-hidden, it leaves a 12px ContextEdge; hovering the edge for 150 ms slides the panel in over the panes, so terminals never resize. The StatusLine runs along the bottom.
- **Scrolling**: the sidebar, every pane and the context panel scroll on their own (`.ao-scroll`). The tab bar scrolls sideways when tabs overflow.
- **Commands** sit in the sidebar under the side tabs: the side tab's own commands, then global ones. A launcher side tab runs its commands when it opens.

Drag a tab within the bar to reorder it, into the sidebar to make a new side tab, onto a side tab to move it there, or outside the window to open a new window, like a browser. Drag a side tab onto another to merge them, contexts included.

## Motion

Arcade cues, short and stepped (`steps()` timing, never ease curves):

- running dots and cells blink in two steps over 960 ms;
- a side tab that starts to need you flashes its row three times;
- the drag ghost tilts 2 degrees and lifts onto `shadow-hard`;
- toasts slide in over `step-slow`;
- buttons drop 3px onto their shadow when pressed (`step-fast`).

All motion stops under `prefers-reduced-motion`. The chime is off by default.

## Keyboard

Plain keys always go to the focused pane: this is a terminal. App actions are `⌘` chords:
- `⌘K` palette
- `⌘T` new tab, `⌘N` new side tab, `⌘D` and `⌘⇧D` split, `⌘W` close
- `⌘1`–`⌘9` switch tab, `⌃⌘1`–`⌃⌘9` switch side tab
- `⌘I` insert a prompt or variable at the cursor
- `⌘E` extract the selection to a variable, `⌘⇧P` pin it
- `⌘⇧S` edit status and colors
- `⌘⇧A` jump to the oldest session waiting on you

Show a Keycap next to every action that has a key.

## Iconography

The app draws its own 8x8 bitmap icons as SVG squares (see Icons). They take `currentColor`, render at 16px (2x) or 8px (1x), and never mix with a stroke icon library. `agent` marks Claude sessions. No emoji, no logos: the product has no mark yet, so its name is set in plain `pixel-display` type.

## Screens this system serves

- The window: sidebar, tabs, split panes, context panel.
- Dragging a tab into the sidebar, onto a side tab, or out of the window.
- Merging side tabs.
- The status and color editor.
- Extracting and pinning from a response.
- Inserting a prompt with its variables.
- The prompt library.
