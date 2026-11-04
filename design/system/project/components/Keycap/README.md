# Keycap

One key drawn as a pixel cap: 2px `line-strong` edges with a 4px bottom edge for depth, the label in `pixel-label`.

Put one next to every action that has a key: buttons, palette rows, the selection toolbar, empty states.

- The app is a terminal, so plain keys always go to the focused pane. App actions use `⌘` chords, which shells and Claude don't read on macOS: `⌘K` palette, `⌘T` tab, `⌘N` side tab, `⌘D` / `⌘⇧D` split, `⌘W` close, `⌘I` insert a prompt or variable, `⌘E` extract the selection, `⌘⇧P` pin it, `⌘1`–`⌘9` switch tab, `⌃⌘1`–`⌃⌘9` switch side tab, `⌘⇧A` jump to the oldest session waiting on you.
- Inside a primary button the cap inverts to `amber-edge` on `on-amber`.
- Classes: `.ao-key`; wrap with a label in `.ao-hint`.
