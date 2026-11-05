# Spec Delta

## ADDED Requirements

### Requirement: Generated statuses
Each tab SHALL have an auto status from hooks and shell integration: Waiting, Running, Done,
Failed or Idle. A side tab's auto status SHALL be its most urgent tab's, in that order. These
built-in statuses SHALL be renamable and recolorable in Settings and SHALL NOT be deletable.

#### Scenario: Group status
- **WHEN** one tab of a side tab is Running and another Waiting
- **THEN** the side tab's auto status is Waiting

### Requirement: Manual statuses and colors
The person SHALL be able to set, on a side tab or a tab, a status label that stays until
"back to auto", an accent from `blue cyan green magenta red fg`, and a background from five
tints. Amber SHALL NOT be offered as an accent or background.

#### Scenario: Override and back
- **WHEN** the person sets `Review` on a side tab and later picks "back to auto"
- **THEN** the row shows `Review` until then, and the generated status after

#### Scenario: Edit from the sidebar chip
- **WHEN** the person clicks the status chip of a side tab or of a tab in the sidebar
- **THEN** the status editor opens on that side tab or tab, with this side tab's and the global statuses to pick from

### Requirement: Needs you shows over manual statuses
The amber needs-you marker SHALL show on a tab and its side tab whenever a Claude session in
it waits, whatever manual status is set.

#### Scenario: Blocked but waiting
- **WHEN** a side tab marked `Blocked` by hand has a Claude session hit a permission prompt
- **THEN** the row shows `Blocked` and the amber marker

### Requirement: Sidebar sort
The sidebar SHALL keep manual order by default and SHALL sort by background, status or name on request.

#### Scenario: Sort by background
- **WHEN** the person sorts by background
- **THEN** side tabs with the same tint sit together, ordered by name inside each tint

### Requirement: Global and local statuses
Settings > Statuses SHALL hold global statuses with label, accent, background and order,
offered on every side tab and tab. A status saved from the editor SHALL be local to its side tab,
including one saved while editing a tab of it, and SHALL offer "make global". Deleting a status in use SHALL return those side tabs and
tabs to their auto status.

#### Scenario: Promote a local status
- **WHEN** the person saves `Waiting on QA` on PROJ-477 and picks "make global"
- **THEN** `Waiting on QA` appears in Settings and in the editor of every side tab

### Requirement: Dynamic statuses
The system SHALL set statuses from rules defined in Settings (a command exit code, Claude idle
time), from `terminescu status` run in any pane, and SHALL clear a status at its expiry (when
the command ends, or after a set time). Precedence SHALL be manual, then rule, then auto.

#### Scenario: Tests red rule
- **WHEN** a rule maps `pnpm test` exiting non-zero to `Tests red` and the command exits with 1 in PROJ-470
- **THEN** PROJ-470 shows `Tests red` in red, unless a manual status is set on it

#### Scenario: Expiring status
- **WHEN** Claude runs `terminescu status "Migrating" --for 20m` in its pane
- **THEN** the tab shows `Migrating` for 20 minutes and returns to its auto status after
