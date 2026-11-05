# Spec Delta

## ADDED Requirements

### Requirement: Side tabs, tabs and panes
A window SHALL show a sidebar of side tabs, the tab bar of the active side tab, and the active
tab's panes. A side tab SHALL hold one or more tabs; with one tab it SHALL draw as a single
row, with two or more as a group that expands to list its tabs. A tab SHALL split into panes
right (`⌘D`) and down (`⌘⇧D`), each pane a terminal, a transcript or a text editor, with
resizable dividers. A side tab SHALL hold any mix of pane kinds.

#### Scenario: Second tab makes a group
- **WHEN** a side tab with one tab gets a new tab with `⌘T`
- **THEN** its sidebar row becomes a group of 2 tabs with a two-cell block strip

#### Scenario: Text tab next to terminal tabs
- **WHEN** the person presses `⌥⌘T` in a side tab with two terminal tabs
- **THEN** a third tab opens as an Untitled text tab and the sidebar group shows 3 tabs

#### Scenario: Split a tab
- **WHEN** the person presses `⌘D` in a terminal pane
- **THEN** the tab shows two panes side by side, the new one running the login shell in the same cwd

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

#### Scenario: Drag a tab into the sidebar
- **WHEN** the person drops a tab on the gap between two side tabs
- **THEN** a new side tab with that tab appears at that position and the old side tab keeps its other tabs

### Requirement: Merge side tabs
Dropping a side tab onto another SHALL move all its tabs into the target and merge the two
contexts. When a variable name exists in both with different values the system SHALL ask to
keep the target's value or keep both with the source renamed; without a clash it SHALL merge
without a dialog and offer undo for 10 s.

#### Scenario: Name clash
- **WHEN** PROJ-477 with `branch=feature/PROJ-477` is dropped onto PROJ-482 with `branch=feature/PROJ-482`
- **THEN** the merge dialog lists `branch` as a clash and keeping both renames the source's to `branch_477`

### Requirement: Layout persistence
Windows, side tabs, tabs, splits, sizes and the active tab SHALL persist, each row with an
`owner_id`, and SHALL be restored
on the next start with each pane reattached to its running shell.

#### Scenario: Restart the app
- **WHEN** the app quits and starts again
- **THEN** every window reopens with the same side tabs, tabs and splits, and each pane shows its live shell
