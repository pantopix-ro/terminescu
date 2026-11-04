# DropZone

Where a dragged tab or side tab can land. Every target is amber while a drag is live and disappears on drop or Esc.

- `.ao-drop-line` between sidebar rows: the tab becomes a new side tab at that position.
- `.ao-st.drop-into` on a row: the tab moves into that side tab. Its context items move with it (see MergeDialog for side tab onto side tab).
- `.ao-drop` at the end of the sidebar: a new side tab at the bottom.
- The TabBar shows `.ao-caret` between tabs for reorder and for tabs arriving from another side tab or window.
- Outside every window, the ghost carries a `New window` chip. Dropping opens a new app window with a default side tab holding the tab, like a browser.
- Esc cancels a drag. Every drag has a palette equivalent ("Move tab to…", "Merge side tab into…") for keyboard use.
