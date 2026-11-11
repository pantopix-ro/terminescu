# shortcuts Specification

## Purpose
Settings › Keys rebinds or turns off any chord, the app's and each plugin's, says when a plugin
loses its chord to another binding, and every hint in the UI shows the chord as bound.

## Requirements

### Requirement: Plugin chords are the person's to change

Settings › Keys SHALL list every action of each enabled plugin under the plugin's name, with
its chord. The person SHALL rebind, reset or turn off each one, and the person's chord SHALL
win over a plugin's default.

#### Scenario: Rebinding a plugin action

- **WHEN** the person binds the git plugin's "New Row from Worktree…" to `⌥⌘W`
- **THEN** the menu shows `⌥⌘W` beside it and the chord runs it

#### Scenario: A taken plugin chord

- **WHEN** a plugin asks for a chord an app action holds
- **THEN** its row in Settings › Keys reads "Wants <chord>, taken by <action>"

#### Scenario: A hidden action keeps its chord

- **WHEN** a plugin action is hidden because its flag is down
- **THEN** no later plugin action takes its chord, and the action has it when the flag rises

### Requirement: Any shortcut turns off

Every app and plugin action SHALL have an "off" choice in Settings › Keys. An action that is off
SHALL keep its menu item with no chord.

#### Scenario: Turning off an app chord

- **WHEN** the person turns off "New Text Tab"
- **THEN** `⌥⌘T` reaches the focused pane and the File menu item shows no chord

### Requirement: The diff layout chord is the app's

"Toggle Diff Layout" SHALL be an app action under View, `⌥⌘U` by default, that switches every
diff tab between side by side and unified, whichever plugin drew it.

#### Scenario: A Claude diff with the git plugin off

- **WHEN** the git plugin is off and a Claude diff tab is focused
- **THEN** `⌥⌘U` switches its layout

### Requirement: Hints show the bound chord

A hint in the UI that names a shortcut SHALL show the chord as bound now, and SHALL leave the
chord out when the action is off.

#### Scenario: A rebind updates the hint

- **WHEN** the person binds "Open File…" to `⇧⌘O`
- **THEN** the file finder's hint reads `⇧⌘O`
