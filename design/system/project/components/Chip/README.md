# Chip

A short state label: an 8x8 icon plus one or two words in `pixel-label`, inside a small notched frame.

- `save` and `wait` fill with `amber` and use `on-amber` text. A save chip appears for 2 s after a prompt, pin or variable is saved, then fades in two steps.
- `run` (blue), `done` (green), `fail` (red) and `pin` (magenta, also for extracted variables) color only the glyph and word.
- `tag` is the quiet variant for prompt tags and filters, in `small` Mono.
- Always pair a status color with its icon and word.
- Side tab statuses use the smaller StatusLabel (`.ao-stat`, see SideTab), whose colors the person can change.
- Classes: `.ao-chip` plus one of `save wait run done fail pin tag`.
