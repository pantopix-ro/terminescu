# Vendored alacritty_terminal 0.25.1

Copied from crates.io (Apache-2.0, see LICENSE-APACHE) for `terminescu-ptyd` (D31).

Two changes. `Term` gains read-only accessors `inactive_grid`, `scroll_region`,
`active_charset`, `keyboard_mode_stack` and `inactive_keyboard_mode_stack` in `src/term/mod.rs`, marked "Terminescu patch".
The daemon serializes a pane's screen from them. Reaching the inactive grid through `swap_alt`
is not an option: swapping back to the alternate screen erases it.

`Grid` gains a `scrolled_rows` counter with the accessor `scrolled_rows()` in `src/grid/mod.rs`,
marked "Terminescu patch". `scroll_up` adds to it in the branch that rotates rows into history.
Every other path into history (clearing the viewport, a resize that shrinks the screen) goes
through `scroll_up`. The daemon anchors prompt marks to `scrolled_rows() + line` so a mark
keeps its row while the grid scrolls.

Left out: upstream's `tests/` folder (46 MB of reference recordings) and its `[[test]]` entry
in `Cargo.toml`.

To update: copy the new release over this folder, reapply the accessors and the counter, and offer them
upstream so the copy can go.
