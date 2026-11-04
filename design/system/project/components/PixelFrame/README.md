# PixelFrame

The notched container: four 2px edges in `line-strong` with open corners, and a blurless `shadow-hard` offset 6px down and right.

Use it for anything that floats above the panes or asks for a decision: dialogs, the palette, the StatusEditor, pin cards, the drag ghost. Panes and side tab rows stay flat.

- Head: an 8x8 icon plus a `pixel-label` of eight characters or fewer. Body in `body`. Optional foot with buttons.
- `amber` variant only when the frame holds something waiting on the person. One amber frame on screen at a time.
- Never round the corners and never blur the shadow. The notch is the corner.
- Classes: `.ao-frame`, `.ao-frame.amber`, `.ao-frame-head`, `.ao-frame-body`, `.ao-frame-foot`.
