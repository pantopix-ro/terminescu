# Spec Delta

## ADDED Requirements

### Requirement: Open a text tab
`⌥⌘T` and `+ ▾ › Text` SHALL open an Untitled text tab, and `⌘O` SHALL search files under the
side tab's cwd, with a second `⌘O` opening the macOS open dialog. `⌘T` SHALL keep opening a
terminal. Holding `⌥` with any opener SHALL split the focused pane instead of adding a tab.

#### Scenario: Untitled tab
- **WHEN** the person presses `⌥⌘T` in PROJ-482
- **THEN** an Untitled text tab opens in PROJ-482 with the cursor in the editor

#### Scenario: Open as a split
- **WHEN** the person holds `⌥` and picks `notes.md` in the `⌘O` search
- **THEN** the focused pane splits and `notes.md` opens beside it

### Requirement: Open a file from elsewhere
`⌘`-click on a file path in terminal output SHALL open it in the same side tab, resolved against
the pane's OSC 7 cwd. `terminescu edit <file>` SHALL open it in the side tab of the calling pane.
A text variable or prompt clicked in the context panel, and a file dropped from Finder on the tab
bar, SHALL open in a text tab.

#### Scenario: Path in terminal output
- **WHEN** `git status` in `~/work/api` prints `src/routes/audit.ts` and the person `⌘`-clicks it
- **THEN** `~/work/api/src/routes/audit.ts` opens in a new text tab of the same side tab

#### Scenario: Drop on a terminal
- **WHEN** the person drops `README.md` and `My Files/it's.txt` from Finder on a terminal pane
- **THEN** the shell's line gets both paths, the second in single quotes, and nothing runs

#### Scenario: Open from a shell
- **WHEN** the person runs `terminescu edit notes/PROJ-482.md` in a pane of PROJ-482
- **THEN** the file opens in a text tab of PROJ-482, even when another window has focus

### Requirement: Editor requests
With the editor setting on in Settings › Shell, which sets `VISUAL` and `EDITOR` to
`terminescu edit --wait` through the shell integration, a program asking for an
editor SHALL get a text pane split beside the pane that asked, marked Waiting (amber) until
`⌘↵` saves and returns or the pane closes.

#### Scenario: Commit message
- **WHEN** `git commit` runs in the shell tab of PROJ-482 with the setting on
- **THEN** a text pane opens beside it with `COMMIT_EDITMSG`, the tab shows the amber marker, and `⌘↵` saves, closes the pane and lets git finish

### Requirement: Save like a text editor
`⌘S` SHALL save the document; for an Untitled buffer it SHALL open the macOS Save dialog. Save As
SHALL be in the File menu and the palette. Closing a tab with unsaved changes SHALL ask Save,
Don't save or Cancel. Unsaved changes SHALL show as a dot on the tab.

#### Scenario: Save an Untitled buffer
- **WHEN** the person types into an Untitled tab and presses `⌘S`
- **THEN** the macOS Save dialog opens and the tab takes the chosen file name once saved

### Requirement: Documents survive restarts
Each text pane SHALL edit one document (file, Untitled buffer, context variable, prompt version
or editor request) held in the core. Unsaved changes and Untitled buffers SHALL survive quitting
and restarting the app, and moving a tab to another window SHALL keep its text. A document
marked session only SHALL never be written to disk.

#### Scenario: Quit with unsaved changes
- **WHEN** the app quits while `review-notes.md` has unsaved edits
- **THEN** the next start shows the same edits with the dirty dot, and the file on disk is unchanged

### Requirement: Changes on disk
The system SHALL watch open files. A clean buffer SHALL reload when its file changes; a dirty
buffer SHALL show a "changed on disk" bar with Reload and Keep mine. Files above 20 MB SHALL open
read-only and binary files SHALL be refused.

#### Scenario: Claude edits an open file
- **WHEN** Claude writes `proposal.md` while it is open with unsaved edits
- **THEN** the pane shows the changed-on-disk bar and nothing is overwritten until the person picks

### Requirement: Editing features in v1
The editor SHALL be CodeMirror 6 and SHALL keep text byte-exact. v1 SHALL offer highlighting for
Markdown and common languages, soft wrap, find and replace, undo, multiple cursors, `{{variable}}`
chips with autocomplete from the side tab's context and a warning on unknown names (secrets
offered only as `$NAME`), pasted images saved next to the file, a status bar (language,
encoding, line endings, line and column), and "Send to pane" (`⌘⇧↵`) as a bracketed paste
without Enter.

#### Scenario: Unknown variable
- **WHEN** a prompt draft contains `{{ticket}}` and `{{tikcet}}` and the context has only `ticket`
- **THEN** `{{ticket}}` renders as a chip and `{{tikcet}}` is underlined with "unknown variable"

#### Scenario: Send to pane
- **WHEN** the person selects a paragraph and presses `⌘⇧↵` with the Claude pane as target
- **THEN** the paragraph lands at Claude's prompt and no Enter is sent

### Requirement: Byte-exact files
Saving SHALL write the file exactly as edited, keeping line endings, trailing whitespace and
encoding.

#### Scenario: CRLF file
- **WHEN** a file with CRLF line endings is opened, one word changed and saved
- **THEN** `git diff` shows one changed line and every line still ends in CRLF
