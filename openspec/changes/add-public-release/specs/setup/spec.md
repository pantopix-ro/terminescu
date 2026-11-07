# Spec Delta

## Purpose
What Terminescu installs outside its own bundle and with whose consent: the first-run screen,
the Claude Code hook consent and toggle, and removing the helpers.

## ADDED Requirements

### Requirement: First-run screen
On the first launch the app SHALL show a screen that explains what Terminescu is, which
helpers it puts outside its bundle (the `terminescu` CLI and the shell daemon in its support
folder, and the Claude Code hook) and what the hook does. The screen SHALL also link the
privacy note, and it SHALL show once.

#### Scenario: First launch
- **WHEN** the app starts with no support folder
- **THEN** the first-run screen opens before the first window takes keys, and it does not open on the next launch

### Requirement: Hook consent
The app SHALL add its Claude Code hook only after the person agrees, on the first-run screen
or with the hook toggle in Settings › Shell. Hook entries already in Claude Code's settings
SHALL count as consent. Without the hook, terminals SHALL work in full, and the Claude status
and transcript features SHALL say that the hook is off. Turning the toggle off SHALL remove
every Terminescu hook entry and leave every other hook and setting as it was.

#### Scenario: Declined
- **WHEN** the person chooses "Not now" on the first-run screen
- **THEN** Claude Code's settings file is unchanged, new tabs open shells, and a Claude pane's status says the hook is off with a button to turn it on

#### Scenario: Owner's existing install
- **WHEN** the app updates on a Mac whose Claude Code settings already hold the Terminescu hook
- **THEN** the hook stays and no consent question appears

#### Scenario: Toggle off
- **WHEN** the person turns the hook off in Settings › Shell
- **THEN** Claude Code's settings file holds no Terminescu entry and every other hook is unchanged

### Requirement: Uninstall helpers
Help › Uninstall Terminescu Helpers SHALL ask first. It SHALL then remove the hook entries,
the copied CLI and daemon binaries, and the shell integration that new shells load. Removing
the support folder, with saved variables, commands and layout, SHALL be a separate opt-in. A
running shell daemon SHALL be stopped only after the person confirms that open shells close.
The app SHALL then quit and say to drag it to the Trash.

#### Scenario: Uninstall keeps data by default
- **WHEN** the person confirms the uninstall without ticking "Also delete my Terminescu data"
- **THEN** Claude Code's settings hold no Terminescu entry, the support folder's bin directory is gone, the database stays, and the app quits
