# ContextEdge

What stays of the context panel when it is set to auto-hide: a 12px strip on the right edge of the window.

- Pips show what the panel holds: magenta for new context items, amber when a pin or variable came from a session that waits.
- Hover the edge for 150 ms (`.hot`) and the panel slides in as `.ao-ctx-over`, on top of the panes with a hard shadow. Terminals keep their size, so no shell or Claude session redraws.
- The panel slides out when the pointer leaves it. `⌘\` switches between pinned and auto-hide.
