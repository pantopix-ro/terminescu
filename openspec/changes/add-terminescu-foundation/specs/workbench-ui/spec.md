# Spec Delta

## ADDED Requirements

### Requirement: Visual tokens
Every color SHALL come from the design tokens, and the xterm theme SHALL be generated from the
same tokens. Amber SHALL mean needs you, focus and drop targets only.

#### Scenario: Theme matches
- **WHEN** the token test compares the xterm theme with the CSS tokens
- **THEN** all 16 ANSI colors match

#### Scenario: Terminal font
- **WHEN** a terminal pane opens
- **THEN** it draws in SF Mono Regular, the system's mono font, at 13 px

### Requirement: Keyboard model
Plain keys SHALL always go to the focused pane. App actions SHALL be `⌘` chords: `⌘K`, `⌘T`,
`⌘N`, `⌘D`, `⌘⇧D`, `⌘W`, `⌘1`-`⌘9`, `⌃⌘1`-`⌃⌘9`, `⌘I`, `⌘E`, `⌘⇧P`, `⌘⇧S`, `⌘⇧A`, `⌘\`,
`⌥⌘T`, `⌘O`. Each SHALL be a native menu item, so the chord wins over the webview and over the
text editor. Every shortcut SHALL be editable in Settings › Keys, which SHALL flag conflicts.

#### Scenario: App chord in a text pane
- **WHEN** a text pane has focus and the person presses `⌘D`
- **THEN** the tab splits and the editor does not select the next match

#### Scenario: Rebind a shortcut
- **WHEN** the person binds "Open file" to `⌘P` in Settings › Keys
- **THEN** `⌘P` opens the file search and `⌘O` does nothing; binding `⌘T` to it as well shows a conflict

#### Scenario: Letter typed in a terminal
- **WHEN** the focused pane is a terminal and the person types `c`
- **THEN** the shell receives `c` and no app action runs

### Requirement: Title strip
The sidebar SHALL start with a strip that holds the macOS traffic lights and the TERMINESCU
wordmark, with no header row under it. The strip and the tab bar SHALL have the same height, and
dragging the strip, the wordmark or the free space of the tab bar SHALL move the window. The strip
and the lights SHALL keep their size on screen at any zoom. A startup error SHALL show an alert
with its message and SHALL be written, with every panic, to `logs/app.log` in the support folder.

#### Scenario: Move the window
- **WHEN** the person drags the wordmark
- **THEN** the window follows the pointer

#### Scenario: A startup error
- **WHEN** the support folder cannot be created at start
- **THEN** an alert names the error and the log file, and the app quits

### Requirement: Zoom
`⌘=` and `⌘+` SHALL zoom every window in one step, fonts and elements and terminal text alike,
`⌘-` SHALL zoom out and `⌘0` SHALL go back to 100%. The zoom SHALL run from 50% to 200% and
SHALL be saved with the settings, so a restart and a new window keep it.

#### Scenario: Zoom in and reset
- **WHEN** the person presses `⌘+` twice and then `⌘0`
- **THEN** the UI shows at 110% and 125%, and then at 100%, with the terminals refitted each time

#### Scenario: Zoom kept
- **WHEN** the person zooms to 125% and quits, then opens the app again
- **THEN** every window opens at 125%

### Requirement: Attention signals
When a session waits, the status line SHALL show the count in amber, the dock icon SHALL show
a badge with the count, and a macOS notification SHALL be posted when the app is in the
background, on by default and without sound. A chime SHALL exist and SHALL be off by default. `⌘⇧A` SHALL focus the pane that
has waited longest.

#### Scenario: Waiting in the background
- **WHEN** a Claude session starts waiting while another app is in front
- **THEN** a notification names the side tab and tab, the dock badge reads 1, and no sound plays

### Requirement: Collapsible context panel
`⌘\` SHALL cycle the context panel between pinned and auto-hide. When auto-hidden, hovering a
6px edge at the right of the window for 150 ms SHALL slide the panel in as an overlay over the
panes without resizing them, and it SHALL slide out when the pointer leaves.

#### Scenario: Peek at the context
- **WHEN** the panel is auto-hidden and the pointer rests on the right edge
- **THEN** the panel slides over the panes and no terminal receives a resize

### Requirement: Adjustable sidebar
The sidebar's right edge SHALL drag its width from 180 to 480 px, saved with the settings. Hovering
the sidebar SHALL show a pixel button that collapses it to the rows' icons and expands it again.
A collapsed row's icon SHALL show a pixel tooltip with the row's name and status on hover, and a
name the row cuts short SHALL show whole in the same tooltip, laid over it.

#### Scenario: Drag the width
- **WHEN** the person drags the sidebar's edge 60 px to the right
- **THEN** the sidebar grows by 60 px, keeps that width after a restart, and a double click on the edge brings back 272 px

#### Scenario: Icons only
- **WHEN** the person hovers the sidebar, clicks the collapse button, then rests the pointer on a row's icon
- **THEN** only the icons show, and a pixel tooltip beside the icon shows the row's name and status

#### Scenario: A long name
- **WHEN** the pointer rests on a side tab or tab name that ends in an ellipsis
- **THEN** a pixel tooltip over the name shows it whole; a name that fits shows none

### Requirement: Independent scrolling
The sidebar, each pane and the context panel SHALL scroll on their own, and the tab bar SHALL
scroll sideways when its tabs overflow.

#### Scenario: Long sidebar
- **WHEN** a window has 30 side tabs
- **THEN** the sidebar scrolls while the tab bar, panes and context panel stay in place

### Requirement: Settings
A Settings screen SHALL offer Statuses, Commands, Variable defaults, Extraction rules
(read-only in v1), Shell (including the opt-in `EDITOR` setting) and Keys (editable).

#### Scenario: Default storage mode
- **WHEN** the person sets the default storage mode to session only in Settings
- **THEN** variables extracted afterwards start as session only
