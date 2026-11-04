# Vendored alacritty_terminal 0.25.1

Copied from crates.io (Apache-2.0, see LICENSE-APACHE) for `terminescu-ptyd` (D31).

The one change: `Term` gains read-only accessors `inactive_grid`, `scroll_region`,
`active_charset`, `keyboard_mode_stack` and `inactive_keyboard_mode_stack` in `src/term/mod.rs`, marked "Terminescu patch".
The daemon serializes a pane's screen from them. Reaching the inactive grid through `swap_alt`
is not an option: swapping back to the alternate screen erases it.

Left out: upstream's `tests/` folder (46 MB of reference recordings) and its `[[test]]` entry
in `Cargo.toml`.

To update: copy the new release over this folder, reapply the accessors, and offer them
upstream so the copy can go.
