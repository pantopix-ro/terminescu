# Spec Delta

## Purpose
Who made Terminescu and on what terms: the About window, the license and credits, the privacy
note, the not-affiliated line and the ways to reach the maker.

## ADDED Requirements

### Requirement: About window
Terminescu › About Terminescu SHALL show the app's name, icon, version and build, "Made by
PANTOPIX RO" with a link to https://pantopix.ro, the license name with a link to its text,
a Licenses button, and the line "Works with Claude Code. Not affiliated with Anthropic."

#### Scenario: About shows maker and version
- **WHEN** the person opens About Terminescu
- **THEN** the window shows the running version and build and "Made by PANTOPIX RO", and clicking pantopix.ro opens it in the default browser

### Requirement: License and credits
The repository and the app SHALL carry the FSL-1.1-Apache-2.0 license text. Help › Licenses
SHALL list every third-party Rust crate and npm package shipped in the app with its license
and the notices its license requires. The list SHALL be generated at build time from the
locked dependencies, and a release SHALL fail when a shipped dependency has a license outside
the allowed set.

#### Scenario: Credits list the shipped libraries
- **WHEN** the person opens Help › Licenses
- **THEN** xterm.js, CodeMirror, Tauri and alacritty_terminal appear with their license names and texts

#### Scenario: Unknown license
- **WHEN** a dependency with a license outside the allowed set enters the lock file
- **THEN** the license check fails and names the package

### Requirement: Privacy note
Help › Privacy and `PRIVACY.md` SHALL state the same facts:
- The app sends no telemetry and no crash reports.
- Its only automatic network call is the update check.
- The hook reads the events Claude Code sends it and the session transcripts on the Mac, and
  nothing leaves the Mac.
- Saved variables live in a local database; secret values live only in memory.
- Feedback leaves the Mac only when the person sends it.

#### Scenario: Privacy from the menu
- **WHEN** the person opens Help › Privacy
- **THEN** the note opens and says the only automatic network call is the update check

### Requirement: Feedback routes
Help › Send Feedback SHALL open a new email in the default mail app to `salut@pantopix.com`
whose subject starts with `[Terminescu]`. Help › Report an Issue SHALL open a new GitHub issue
on the public repository. Both SHALL be pre-filled with the app version and build and the
macOS version, and nothing SHALL be sent without the person's own send action.

#### Scenario: Email feedback
- **WHEN** the person chooses Send Feedback and continues without ticking the box
- **THEN** the mail app opens a draft to salut@pantopix.com with subject "[Terminescu] Feedback" and the versions in the body, and no log

#### Scenario: Issue with the log
- **WHEN** the person chooses Report an Issue and ticks "Include app.log"
- **THEN** the browser opens a new issue with the versions filled in, and Finder shows app.log ready to drag into it

### Requirement: Log only on request
Before either feedback route opens, a dialog SHALL offer an "Include app.log" box, off by
default, and say that the log can hold file paths. The log SHALL go along only when the box
is ticked.

#### Scenario: Log left out
- **WHEN** the person sends feedback without ticking the box
- **THEN** no log is attached or revealed
