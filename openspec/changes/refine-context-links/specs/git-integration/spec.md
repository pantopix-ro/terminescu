## ADDED Requirements

### Requirement: PR links in the context
While GitHub is on, the git plugin SHALL look up each GitHub PR link added to a side tab's
context with `gh`, name the link with the PR title as a tool source, and report the PR's
state, open, draft, merged or closed, and its checks result as a badge on the row. The plugin
SHALL look a PR up at most once every two minutes, and again when its side tab is selected
after that. While GitHub is off, no badge SHALL show and no lookup SHALL run.

#### Scenario: Open PR with failing checks
- **WHEN** a side tab gets the link to open PR 918, titled "Retry failed payments", whose checks fail
- **THEN** the row reads "Retry failed payments" with an "open" badge and a failed-checks mark

#### Scenario: Merged later
- **WHEN** the PR merges and the person selects the side tab three minutes later
- **THEN** the badge reads "merged"

#### Scenario: gh logged out
- **WHEN** `gh` is logged out
- **THEN** PR rows show their names or labels with no badge, and the plugin runs no lookups
