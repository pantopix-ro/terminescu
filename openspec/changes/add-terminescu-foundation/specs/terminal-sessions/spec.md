# Spec Delta

## ADDED Requirements

### Requirement: Shells owned by a daemon
Each terminal pane SHALL run in a pty owned by the `terminescu-ptyd` daemon, started with the
person's login shell, the pane's cwd and `TERMINESCU_PANE_ID` in its environment. Closing a window,
quitting the app or upgrading it SHALL NOT end the shells; closing a tab or pane SHALL. The app
SHALL start the daemon detached, from a copy outside the app bundle, and SHALL check the
protocol version on every connection.

#### Scenario: Window closed with a running server
- **WHEN** the person closes a window whose tab runs `pnpm dev` and reopens it from the window list
- **THEN** `pnpm dev` is still running and its output is continuous

### Requirement: Reattach with scrollback
The daemon SHALL keep a screen model of each pane (at most 10,000 lines of history by default),
SHALL send a snapshot of it when a view reattaches to the pane, and the view SHALL then resize to
its own size so full-screen programs redraw.
Moving a tab to another window and restarting the app SHALL both use this reattach.

#### Scenario: Reattach after restart
- **WHEN** the app restarts while a pane has 500 lines of output
- **THEN** the pane shows those 500 lines and the live prompt

#### Scenario: Tab moved to another window
- **WHEN** a tab running `claude` is dropped into a second window
- **THEN** the second window shows its scrollback and a redrawn Claude screen, and the session never restarts

#### Scenario: Long Claude session
- **WHEN** a pane has run `claude` for hours and produced far more than 10,000 lines of output
- **THEN** a reattached view shows Claude's current screen in the alternate buffer, and the shell history above it

#### Scenario: App upgraded with a newer daemon
- **WHEN** the app updates to a build with daemon protocol 2 while panes run on daemon protocol 1
- **THEN** the app talks protocol 1 to the running daemon and offers "Restart sessions" to move to the new one

### Requirement: New tabs start the login shell
A new tab or pane SHALL start `$SHELL -l` in the cwd of the focused pane, never `claude`.

#### Scenario: New tab
- **WHEN** the person presses `⌘T` while the focused pane's cwd is `~/work/api`
- **THEN** the new tab runs the login shell in `~/work/api`

#### Scenario: Colored ls by default
- **WHEN** a new shell starts and neither `CLICOLOR` nor `NO_COLOR` is set
- **THEN** the shell gets `CLICOLOR=1`, so BSD `ls` prints colors; a value the person set wins

### Requirement: Shell integration
The daemon SHALL inject an integration for zsh, bash and fish that reports prompt, command
start, command end with exit code (OSC 133), the command line, and cwd (OSC 7). It SHALL run
after the person's rc files, SHALL set `VISUAL` and `EDITOR` when the editor setting is on, and
SHALL load the side tab's session and secret variables before each prompt. The tab title SHALL
be the running command, else the shell and cwd.

#### Scenario: Failed command
- **WHEN** `pnpm test` exits with code 1 in a tab
- **THEN** that tab's auto status is Failed and its title returns to `zsh · ~/work/api`

### Requirement: Reboot recovery
After a reboot the app SHALL restore every window, side tab, tab and split, starting a fresh
login shell in each pane's last cwd.

#### Scenario: Reboot
- **WHEN** the Mac reboots with 3 side tabs open
- **THEN** the app reopens the same layout with login shells in the panes' last folders

### Requirement: Command line tool
A `terminescu` CLI SHALL ship inside the app with subcommands `hook`, `status`, `var` (with `env`) and `edit`, and
SHALL find its pane through `TERMINESCU_PANE_ID`. The app SHALL keep a copy of it at a path that
survives app updates, first on each pane's `PATH`. `var env` SHALL answer from the daemon, so it
works while the app is closed.

#### Scenario: Status from a script
- **WHEN** a deploy script runs `terminescu status "Deploying" --color cyan --until-exit`
- **THEN** the tab shows `Deploying` in cyan until the script's command ends, then its auto status

#### Scenario: Variables while the app is closed
- **WHEN** the app is closed and a pane's shell shows its next prompt
- **THEN** the shell still loads its side tab's session and secret variables from the daemon
