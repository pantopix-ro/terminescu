# TabBar

The tabs of the active side tab, across the top like browser tabs. A tab holds a terminal, a text editor or a transcript, and can split into panes of any kind.

- A tab shows a status dot, an `agent` icon when a Claude session runs in it, the title (the running command, else `shell · cwd`), and a close button.
- The active tab joins the pane area below it: `panel` fill, no bottom edge, and a 2px top line in the side tab's accent.
- `+` or `⌘T` opens a new terminal tab running the default shell, not Claude. Claude starts by typing `claude` or from the palette.
- The `▾` half of the split button (`.ao-tab-split`) opens the new-tab menu: Terminal, Text, Open file. Text tabs carry the `doc` icon and a filled square (`.dirty`) in place of the close button while they hold unsaved changes.
- Drag inside the bar to reorder: the tab lifts as a `.ghost` with the notched frame, a dashed `.placeholder` holds its old slot, and the amber `.ao-caret` marks the drop point.
- Drag a tab into the sidebar to make a new side tab. Drag it outside the app window to open a new window holding a new side tab with that tab, like a browser.
- Titles truncate at the end. Over 10 tabs, the bar scrolls sideways and the sidebar row still lists them all.
- Classes: `.ao-tabs`, `.ao-tab` (+ `.active`, `.ghost`, `.placeholder`), `.ao-tab-new`, `.ao-tab-split`, `.ao-caret`, `.dirty`.
