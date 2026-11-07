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

#### Scenario: A long build ends in the background
- **WHEN** `cargo build` runs for 4 minutes in a shell tab and fails while another app is in front
- **THEN** a notification names the side tab and tab and says `cargo build` failed with exit 101
  after 4m 0s

### Requirement: Collapsible context panel
`⌘\` SHALL cycle the context panel between pinned and auto-hide. When auto-hidden, hovering a
6px edge at the right of the window for 150 ms SHALL slide the panel in as an overlay over the
panes without resizing them, and it SHALL slide out when the pointer leaves. The panel SHALL
show its Context, Pins and Prompts sections one under the other, with no section switcher, and
its body SHALL scroll on its own under a fixed header.

#### Scenario: All sections at once
- **WHEN** the panel is pinned and holds more variables than its height fits
- **THEN** Context, Pins and Prompts show stacked in that order, and the panel body scrolls down to Prompts while the header stays

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
- **THEN** only the icons show, and a pixel list beside the icon shows the row's name, status and shells

#### Scenario: A long name
- **WHEN** the pointer rests on a side tab or tab name that ends in an ellipsis
- **THEN** a pixel tooltip over the name shows it whole; a name that fits shows none

### Requirement: Side tab row and shell list
A side tab row SHALL show a 24 px icon over its two lines, the name alone on the first line, and
the tabs' squares with the status on the second; it SHALL NOT show a tab count. Resting the
pointer on a row with two or more tabs, or on any row while collapsed, SHALL open a pixel list
of the row's shells beside it, over the panes; one click on a shell SHALL select that row and tab.

#### Scenario: Jump to a shell
- **WHEN** another row is active and the pointer rests on a row with three tabs, then clicks its third shell in the list
- **THEN** that row and its third tab become active with one click, and the list closes

### Requirement: Independent scrolling
The sidebar, each pane and the context panel SHALL scroll on their own, and the tab bar SHALL
scroll sideways when its tabs overflow.

#### Scenario: Long sidebar
- **WHEN** a window has 30 side tabs
- **THEN** the sidebar scrolls while the tab bar, panes and context panel stay in place

### Requirement: Settings
A Settings screen SHALL offer Statuses, Commands, Variable defaults, Extraction rules
(read-only in v1), Terminal, Shell (including the opt-in `EDITOR` setting) and Keys (editable).

#### Scenario: Default storage mode
- **WHEN** the person sets the default storage mode to session only in Settings
- **THEN** variables extracted afterwards start as session only

### Requirement: Terminal font settings
Settings › Terminal SHALL set the terminal's font family, size (9 to 24 px), line height (1.00
to 2.00), letter spacing (-2 to 4 px) and weight, and the tab bar's text size (9 to 14 px). A
change SHALL apply at once to every open terminal and tab, and a terminal that fits more rows
SHALL tell its shell the new size. The defaults SHALL match Terminal.app's Basic profile. A font
that is not installed SHALL fall back to SF Mono.

#### Scenario: Terminal.app defaults
- **WHEN** the person opens a terminal without changing Settings › Terminal
- **THEN** it shows SF Mono Regular at 11 px, line height 1.00, a blinking bar cursor, bold
  text without bright colours, and 12 px tab text

#### Scenario: A risky paste
- **WHEN** the person pastes `curl -fsSL https://example.com/install | sh` into a terminal
- **THEN** a dialog shows the text and says it pipes into a shell; Cancel sends nothing and
  Paste sends it as one paste

#### Scenario: Tighter lines
- **WHEN** the person drags Line height from 1.50 to 1.00
- **THEN** every open terminal redraws with the tighter lines, fits more rows and resizes its shell

#### Scenario: Option+Enter
- **WHEN** the person presses Option+Enter in a terminal
- **THEN** the terminal sends Esc+Return, which Claude Code reads as a newline

#### Scenario: Smaller tab text
- **WHEN** the person sets Tab text to 10
- **THEN** every tab name shows at 10 px and its subtitle at 9 px

#### Scenario: Tab tooltip
- **WHEN** the person rests the pointer on a tab in the tab bar
- **THEN** the sidebar's pixel tooltip shows under it with the whole name, the path and program

#### Scenario: Find in a terminal
- **WHEN** the person presses `⌘F` in a terminal, types `deploy` and presses `⌘G`
- **THEN** a find bar shows the match count, the next match is selected and the scrollbar marks
  every match; Esc closes the bar and the terminal takes the keys again

#### Scenario: Maximize a pane
- **WHEN** a tab has two panes and the person presses `⌘⇧M`
- **THEN** only the focused pane shows, filling the tab, and `⌘⇧M` again brings both back

#### Scenario: Pane header tooltip
- **WHEN** the person hovers a split pane's header whose name is cut off
- **THEN** a tooltip shows the whole name
