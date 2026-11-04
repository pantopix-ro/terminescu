# Toast

A typed notice in the bottom-right corner: icon, a `pixel-label` type word, one line of Mono, and an optional Keycap for the obvious next action.

- Types: `wait` (amber frame, stays until handled), `done`, `pin` (pins and extracted variables), `fail`. Only `wait` uses an amber frame.
- `done` and `pin` leave after 4 s. `fail` and `wait` stay until acted on or dismissed.
- Name the side tab first, then the tab. One line; truncate the middle of long paths.
- When the app is in the background, `wait` and `fail` also post a macOS notification. The chime is off by default.
