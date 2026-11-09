# SideTab

One row in the sidebar. A side tab is a terminal window that lives inside the app: it holds one or more tabs, and one context (variables, pins, links, prompts) shared by all of them.

- One tab: one line at a folder tab's height (`.ao-st.single`): icon, name, the cwd beside it, and the status chip only when one is set or under the mouse.
- Two or more tabs: a folder. The meta line shows a BlockStrip with one cell per tab and the tab count. The chevron or a click opens the folder and lists its tabs as `.ao-st-kid` rows, with `⌘1` to `⌘9` hints.
- Accent: a 4px left edge in the hue the person picked (`--st-accent`). Background: one of the `tint-*` tokens (`--st-bg`). Both live in the StatusEditor. Defaults are `line-strong` and `panel`.
- Status: an `.ao-stat` label. Auto statuses come from the sessions (Running, Waiting, Done, Failed, Idle). A status set by hand gets `.manual` (a 4px square before the word) and stays until the person clears it.
- Needs you: `.need` on the row adds the amber corner square and flashes the row three times. Amber always shows when a Claude session waits, even when the person set their own status. The person can't hide it.
- Active side tab: a 2px `fg` inset frame. The sidebar is 272px wide and can collapse to a 48px rail of accent edges and dots.
- Drag a row onto another row to merge the two side tabs (see MergeDialog). Drag a tab from the TabBar into the sidebar to make it a side tab of its own (see DropZone).
- Classes: `.ao-side`, `.ao-st` (+ `.active`, `.need`, `.drop-into`), `.ao-st-kids`, `.ao-st-kid`, `.ao-stat` (+ `.need`, `.manual`), `.ao-dot` (+ `need run done fail idle`).
