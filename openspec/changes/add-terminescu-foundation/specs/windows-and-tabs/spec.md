# Spec Delta

## ADDED Requirements

### Requirement: Side tabs, tabs and panes
A window SHALL show a sidebar of side tabs, the tab bar of the active side tab, and the active
tab's panes. A side tab SHALL hold one or more tabs; with one tab it SHALL draw as a single
row, with two or more as a group that expands to list its tabs. A tab SHALL split into panes
right (`⌘D`) and down (`⌘⇧D`), each pane a terminal, a transcript or a text editor, with
resizable dividers. A side tab SHALL hold any mix of pane kinds.

#### Scenario: A new terminal sits at the selected row's level
- **WHEN** the person presses `⌘T` with a one-tab row selected, and again with a tab of a folder selected
- **THEN** the first opens a one-tab row right below the selected row, and the second adds a tab to that folder; `⌘⇧N` always adds a row at the end

#### Scenario: New window
- **WHEN** the person presses `⌘N`
- **THEN** a new window opens with one row holding one new terminal tab, and the other windows keep their rows

#### Scenario: A row dropped on a row makes a folder
- **WHEN** the person drags a one-tab row onto another one-tab row
- **THEN** the two rows become one folder of 2 tabs

#### Scenario: Text tab next to terminal tabs
- **WHEN** the person presses `⌥⌘T` in a side tab with two terminal tabs
- **THEN** a third tab opens as an Untitled text tab and the sidebar group shows 3 tabs

#### Scenario: Add a side tab from the empty space
- **WHEN** the pointer rests on the empty sidebar space under the last side tab
- **THEN** a `+` shows there, and a click on it adds a terminal at the selected row's level, as `⌘T` does

#### Scenario: Rename with a double click
- **WHEN** the person double-clicks the name of a side tab or of a tab, in the sidebar or the tab bar, types `PROJ-482` and presses Enter
- **THEN** the name changes in place; Esc keeps the old name, and only the field double-clicked opens

#### Scenario: New side tabs get their own neon
- **WHEN** the person adds three side tabs, or drags a tab out to a row of its own
- **THEN** each new row opens with a random neon from cyan, pink, lime, violet, mint, blue, rose and ice that no other row uses yet, on its left edge and its icon, washed 12% into the row's background (over a status background when one is set)

#### Scenario: Tabs get their own neon
- **WHEN** the person opens three tabs in a side tab
- **THEN** each tab's icon takes a neon its side tab's other tabs don't use, and the row shows one square per tab in that tab's neon under its name; in the tab bar the tab shows its icon in that neon, washed 12% into its background, active or not; rows and tabs saved before neons or icons get them at the next start

#### Scenario: New tabs get their own icon
- **WHEN** the person opens three tabs
- **THEN** each opens with a random icon from the list that no other tab uses yet, and a one-tab side tab row shows its tab's icon

#### Scenario: Pick an icon
- **WHEN** the person double-clicks the icon of a side tab or of a tab, or picks "Change icon…" in its menu
- **THEN** a list of 20 predefined icons opens; a pick replaces the default icon and persists, and "Default icon" brings the old one back, Claude's state dot staying either way

#### Scenario: Split a tab
- **WHEN** the person presses `⌘D` in a terminal pane
- **THEN** the tab shows two panes side by side, the new one running the login shell in the same cwd

### Requirement: Tab names
Each new tab SHALL get a random animal or bird name that no tab in any window has, saved
with the tab; a number SHALL follow it only once every name is taken. A name typed by the
person SHALL override it, and clearing that name SHALL bring it back. A side tab without its
own name SHALL show its first tab's.

#### Scenario: Random names
- **WHEN** the person opens three tabs and drags one to another window
- **THEN** each tab shows its own name, such as Otter, Heron and Wren, and no two tabs in any window share one

### Requirement: Live tab subtitles
Each tab SHALL show a live subtitle from its focused terminal: the path from home, then the
running program's name, or `claude` and Claude's title while Claude runs there. A program's
arguments SHALL never show.

#### Scenario: Subtitle follows the terminal
- **WHEN** `TOKEN=tok-example npm run dev` runs in `~/work/api`, then ends, and `claude` starts there with the title "◐ Fix PROJ-482"
- **THEN** the subtitle reads `~/work/api · npm`, then `~/work/api`, then `~/work/api · claude · Fix PROJ-482`, without the spinner or the token

### Requirement: Drag tabs like a browser
The person SHALL be able to drag a tab within its tab bar to reorder it, onto a sidebar gap to
make a new side tab at that position, onto a side tab row to move it there, onto another
window's tab bar or sidebar to move it into that window, and outside every window to open a
new window holding a new side tab with that tab. A window left with no side tab SHALL close,
unless it is the last one. Esc SHALL cancel a drag, and
each drag SHALL have a palette equivalent.

#### Scenario: Drag a tab out of the window
- **WHEN** the person drops the `pnpm dev` tab outside the app window
- **THEN** a new window opens at the cursor with one side tab holding that tab, and its shell keeps running

#### Scenario: Drag a tab into another window
- **WHEN** the person drags a tab over a second window's side tab row and releases
- **THEN** the row lights while the cursor is over it, the tab moves into that side tab, and a source window left empty closes

#### Scenario: Merge all windows
- **WHEN** three windows are open and the person presses `⌃⌘M` in the first
- **THEN** every row of the other two joins the end of the first window's sidebar with its context, and the other two windows close

#### Scenario: Windows reopen where they were
- **WHEN** the person moves one window to the external screen, quits, unplugs that screen and reopens Terminescu
- **THEN** the other window reopens where it was, and the moved one opens at the main screen's top left, shrunk to fit it

#### Scenario: Drag a window's only row back
- **WHEN** a second window holds one side tab with one tab and the person drags that row from its sidebar into the first window and releases
- **THEN** the row joins the first window with its context and the second window closes

#### Scenario: Reopen a closed row
- **WHEN** the person closes row `PROJ-482`, which has two tabs and the variable `ticket`, and presses `⇧⌘T`
- **THEN** the row comes back at its place with both tabs, its icon, accent, status and `ticket`, and each pane starts a fresh login shell in its last folder

#### Scenario: Drag a folder to another window
- **WHEN** the person drags a folder row with three tabs onto a row of another window and releases
- **THEN** the folder merges into that row, asking first when names clash; released on a gap or the panes it joins that window as a row, and released outside every window it opens a new window at the cursor

#### Scenario: Drag a tab into the sidebar
- **WHEN** the person drops a tab on the gap between two side tabs
- **THEN** a new side tab with that tab appears at that position and the old side tab keeps its other tabs

### Requirement: Dock a tab beside a pane
A tab dragged over a pane of the shown tab SHALL preview the half it takes, picked by the
triangle under the pointer, and on release SHALL split that pane 50/50 with the tab's panes,
keeping their shells and context. The middle of the pane, the shown tab itself and a pane too
small to halve SHALL change nothing. A split pane's header dragged to the tab bar SHALL make
that pane a tab.

#### Scenario: Drop on the right edge
- **WHEN** the person drags the tab `acme/api` onto the right edge of the shown pane
- **THEN** an amber preview covers the right half, and on release the pane and `acme/api`'s shell sit side by side and `acme/api` leaves the tab bar

#### Scenario: Drop in the middle
- **WHEN** the person releases the tab over the middle of a pane, or presses Esc first
- **THEN** no pane splits and the tab stays where it was

#### Scenario: Pane back to a tab
- **WHEN** the person drags a split pane's header to the end of the tab bar
- **THEN** that pane becomes a new tab there, with its shell, and the other panes fill its space

#### Scenario: Pane back to a tab by keyboard
- **WHEN** the focused pane shares its tab and the person presses ⌘⇧B
- **THEN** that pane becomes a new tab right after the current one; in a tab with one pane ⌘⇧B does nothing

### Requirement: Merge side tabs
Dropping a side tab onto another SHALL move all its tabs into the target and merge the two
contexts. When a variable name exists in both with different values the system SHALL ask to
keep the target's value or keep both with the source renamed; without a clash it SHALL merge
without a dialog and offer undo for 10 s.

#### Scenario: Name clash
- **WHEN** PROJ-477 with `branch=feature/PROJ-477` is dropped onto PROJ-482 with `branch=feature/PROJ-482`
- **THEN** the merge dialog lists `branch` as a clash and keeping both renames the source's to `branch_477`

### Requirement: Sidebar right-click menus
A right click, Shift F10 or the menu key on a side tab, or on a tab in the sidebar or the tab bar,
SHALL open that element's edit dialog: name, icon, colour from the neons, status, and its other
actions. On a saved command or the empty list it SHALL open an in-app menu, where arrow keys
move, Enter runs an item and Esc closes it.

#### Scenario: Rename a side tab
- **WHEN** the person right-clicks a side tab, types `PROJ-482` as its name and presses Enter
- **THEN** the row shows `PROJ-482`; Esc keeps the old name, and a blank name goes back to its first tab's name

#### Scenario: Pick a row colour
- **WHEN** the person right-clicks PROJ-482 and picks lime under Row colour
- **THEN** the row's edge and icon turn lime at once and stay lime after a restart

#### Scenario: Side tab actions
- **WHEN** the person opens a side tab's edit dialog
- **THEN** it offers new tab, merge into, move to new window and close beside its status, and close asks first since the context goes with it

#### Scenario: Move a side tab to a new window
- **WHEN** the person picks "Move to new window" on PROJ-482
- **THEN** a new window opens holding PROJ-482 with its tabs and its context

#### Scenario: Tab and command actions
- **WHEN** the person right-clicks a tab, or a saved command
- **THEN** a tab's dialog offers its name, icon, colour, status, move to and close; a command's menu offers run, insert, restart when down, edit (keeping its scope), duplicate and delete

### Requirement: Layout persistence
Windows, side tabs, tabs, splits, sizes and the active tab SHALL persist, each row with an
`owner_id`, and SHALL be restored
on the next start with each pane reattached to its running shell.

#### Scenario: Restart the app
- **WHEN** the app quits and starts again
- **THEN** every window reopens with the same side tabs, tabs and splits, and each pane shows its live shell
