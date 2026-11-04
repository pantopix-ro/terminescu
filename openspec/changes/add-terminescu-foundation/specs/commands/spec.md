# Spec Delta

## ADDED Requirements

### Requirement: Saved commands
The system SHALL store shell commands with `{{variable}}` placeholders, scoped to one side tab
(in its context) or global (in the library), and SHALL list them in a Commands section of the
sidebar. "Run" SHALL paste the resolved command and send Enter; "Insert" SHALL paste it without
Enter. The target SHALL be a new tab, the focused pane or a new side tab.

#### Scenario: Run in a new tab
- **WHEN** the person runs `gh pr checks {{pr_api}}` from PROJ-482's Commands with `pr_api` set
- **THEN** a new tab opens in PROJ-482 and runs the command with the PR URL filled in

#### Scenario: Missing variable
- **WHEN** a command uses `{{pr_api}}` and the side tab has no `pr_api`
- **THEN** the system asks for the value before running

### Requirement: Launcher side tabs
A side tab SHALL be able to act as a launcher defined by commands. A launcher SHALL run each
command in its own tab when opened and after a reboot, SHALL offer restart for one or all, and
SHALL derive its status from those processes.

#### Scenario: Dev servers after a reboot
- **WHEN** the Mac reboots and the launcher `dev servers` holds `docker compose up` and `pnpm dev`
- **THEN** both commands start in their tabs and the row reads `2/2 up` once both run

#### Scenario: One process dies
- **WHEN** `pnpm dev` exits with code 1 in the launcher
- **THEN** the row reads `1/2 up` in red and offers restart
