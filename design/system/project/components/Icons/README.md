# Icons

The app's own icon set: 8x8 bitmaps drawn as SVG squares with `shape-rendering: crispEdges`, filled with `currentColor`. Shown at 16px (2x) by default, 8px (1x) in dense rows.

Set: pin, check, cross, save, doc, bp, play, link, dot, term, pr, tpl, agent, tab, group, split, window, var, out, in, merge, drag, plus, chev, chevr, prompt.

- In markup, `<i class="ao-i ao-i-<name>"></i>` paints the same bitmap as a CSS mask.
- `agent` marks a tab or pane running a Claude session. `group` a side tab with several tabs, `term` one with a single shell.
- Draw new icons on the same 8x8 grid, one-pixel strokes, no anti-aliasing. Never mix in a stroke icon library. No emoji, no logos.
