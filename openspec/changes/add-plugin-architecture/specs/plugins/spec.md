## Purpose

Built-in plugins add integrations such as Claude Code and git to Terminescu. The terminal,
the sidebar, tabs, panes, context and text tabs work with every plugin switched off.

## ADDED Requirements

### Requirement: Built-in plugins with a switch each
Terminescu SHALL ship its integrations as built-in plugins, compiled into the app. Settings ›
Plugins SHALL list each plugin with its name, a one-line description and a switch, on by
default. A plugin switched off SHALL get no events, SHALL show no decorations, panel sections,
pane views or actions, and SHALL come back with its saved settings when switched on. The app
SHALL NOT load plugin code from outside its bundle.

#### Scenario: Switch the git plugin off
- **WHEN** the person switches Git off in Settings › Plugins
- **THEN** the branch chips leave every row and tab, the Changes section leaves the context
  panel, and every shell keeps running

#### Scenario: Switch it back on
- **WHEN** the person switches Git on again
- **THEN** the chips come back within 2 seconds and the worktree folder setting is the one saved
  before

### Requirement: The app works with no plugin
With every plugin switched off, or built without plugins, Terminescu SHALL open windows, rows,
tabs, split panes, terminals and text tabs, keep context and prompts, and restore its layout.
Nothing a plugin adds SHALL be needed to start, type, split, save or quit.

#### Scenario: No plugins
- **WHEN** the app starts with every plugin switched off
- **THEN** the first window opens with its rows, the shells reattach with their scrollback, and
  no row turns amber

### Requirement: Plugins never hold up the terminal
A plugin SHALL run off the terminal's input and output path. A plugin call or event that
errors, panics or takes longer than its budget SHALL be dropped and logged in `app.log`, and a
plugin that fails 5 times in a minute SHALL be paused and marked "Stopped after errors" in
Settings › Plugins until the person switches it on again.

#### Scenario: A plugin hangs
- **WHEN** a plugin's event handler blocks for 10 seconds
- **THEN** typing in every terminal echoes at once, the event is dropped after its budget, and
  `app.log` names the plugin and the event

#### Scenario: A plugin keeps failing
- **WHEN** a plugin panics on 5 events within a minute
- **THEN** it gets no more events and Settings › Plugins shows it as stopped after errors

### Requirement: Plugins fill the app's slots
A plugin SHALL be able to add chips, subtitle parts and badges to rows and tabs, sections to the
context panel, kinds of pane view, items to the menus and the palette with optional chords, facts that
dynamic status rules can test, read-only variables for prompts, and a page to Settings. The app SHALL show each contribution only while its plugin is on, and a
chord a plugin asks for SHALL lose to one the app or the person already uses.

#### Scenario: Two plugins decorate one row
- **WHEN** the git plugin adds a branch chip and the Claude plugin adds a context meter to the
  same tab
- **THEN** the tab shows both, in plugin order, and its name keeps at least 6 characters

#### Scenario: A chord clash
- **WHEN** a plugin asks for `⌘T`
- **THEN** `⌘T` keeps opening a terminal and the plugin's action shows in the palette with no
  chord

### Requirement: Agents come from adapters
The amber row, the attention count, the dock badge, notifications, `⌘⇧A`, generated statuses and
resume after reboot SHALL work from a generic agent state (idle, running, waiting, done) that an
agent adapter provides. The Claude plugin SHALL provide the adapter for Claude Code, with every
`claude-awareness` requirement unchanged.

#### Scenario: Claude waits, through the plugin
- **WHEN** a Claude session in a tab asks for permission
- **THEN** its row turns amber, the dock badge reads 1, and `⌘⇧A` focuses that pane, as before
  this change

#### Scenario: The Claude plugin is off
- **WHEN** the Claude plugin is switched off and a Claude session asks for permission
- **THEN** the terminal shows Claude's prompt as any program's output and no row turns amber

### Requirement: A pane view without its plugin
A pane whose view belongs to a plugin that is off SHALL keep its place in the layout and SHALL
show the plugin's name and a button that opens Settings › Plugins.

#### Scenario: A transcript pane with Claude off
- **WHEN** a tab holds a transcript pane and the Claude plugin is switched off
- **THEN** the pane shows "Needs the Claude plugin" with an Open Settings button, and the split
  keeps its sizes
