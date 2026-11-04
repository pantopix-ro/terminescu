# PixelButton

Square button with a notched frame and a 3px hard shadow. Pressing drops the face 3px onto its shadow.

- `primary` (amber fill, `on-amber` text, `amber-edge` notch): the one action a dialog is for. At most one per view.
- default: neutral actions on `panel-2`.
- `ghost`: dismiss and cancel, no frame.
- `danger`: destructive actions (close a tab with a running process, kill a session), red label only.
- Labels start with a verb in sentence case, Geist Mono 600. Add the icon when it helps scanning and a Keycap when a key exists.
