## ADDED Requirements

### Requirement: Clean links
Every link added to a side tab's context SHALL pass one cleaner, whatever its source. The
cleaner SHALL remove credentials and tracking parameters (list in design D2), lowercase the host, drop the default port and a trailing slash, and cut GitHub PR and
claude.ai artifact URLs to their base form. It SHALL keep other query parameters and anchors.
A value that is not an http(s) URL SHALL be refused as a link.

#### Scenario: Credentials and tracking go
- **WHEN** the person pastes `https://me:tok@Docs.Example.test:443/guide/?utm_source=mail&page=2#setup`
- **THEN** the side tab holds `https://docs.example.test/guide?page=2#setup`

#### Scenario: PR sub-page
- **WHEN** Claude's reply holds `https://github.com/acme/shop-api/pull/918/files#diff-3a`
- **THEN** the side tab holds `https://github.com/acme/shop-api/pull/918`

#### Scenario: Not a link
- **WHEN** the person adds `ftp://files.example.test/a` as a link
- **THEN** it is refused with "is not an http(s) link"

### Requirement: One row per cleaned link
Two links SHALL be the same link when their cleaned forms match. Adding a link the side tab
already holds SHALL keep the existing row and its place, and SHALL only upgrade its name. The
first start after this change SHALL clean every saved link once, so that copies show as one
row with the best name. Copies that move with their tab SHALL stay.

#### Scenario: Two forms of one PR
- **WHEN** `.../pull/918` and then `.../pull/918/commits` are added to one side tab
- **THEN** the Links head counts 1

#### Scenario: Saved copies merge once
- **WHEN** a database from before this change holds `https://example.test/a/` and `https://example.test/a?utm_source=x` in one side tab and the app starts
- **THEN** the side tab shows one link, `https://example.test/a`, and a second start changes nothing

### Requirement: Link names
A link SHALL carry the name from its strongest source: a tool (the `Artifact` call, or the PR
title from `gh`) over `[Name](url)` text in a reply or terminal output, over none. A name SHALL
replace one from the same or a weaker source only. Link text that is a URL or a filler word
SHALL not count. A link without a name SHALL show a label from its URL (design D6).

#### Scenario: Labels
- **WHEN** a side tab holds an unnamed PR, artifact and docs link
- **THEN** they read `PR #918 · shop-api`, `Artifact` and `docs.example.test · guide`

#### Scenario: Artifact title from the tool call
- **WHEN** Claude publishes an artifact titled "Release checklist" and its URL appears in the reply
- **THEN** the row reads "Release checklist"

#### Scenario: Reply text does not override a tool name
- **WHEN** the reply later writes `[here](<that artifact URL>)`
- **THEN** the row still reads "Release checklist"

#### Scenario: Unnamed PR
- **WHEN** a PR link is added while GitHub is off
- **THEN** the row reads `PR #918 · shop-api`

### Requirement: Link rows
The section SHALL be titled "Links". Each link row SHALL show an icon for its type, PR,
artifact or web, then its name or label on one line, cut at the end. Hovering or focusing the
row SHALL show the panel tooltip with the full name and the cleaned URL. A left click SHALL
open the link. A right click SHALL show a menu with Open, Copy link, Copy as Markdown and
Remove. Copy as Markdown SHALL copy `[<name or label>](<url>)`.

#### Scenario: Tooltip
- **WHEN** the person hovers the "Release checklist" row
- **THEN** a panel tooltip shows "Release checklist" and the artifact URL, and no browser tooltip shows

#### Scenario: Copy as Markdown
- **WHEN** the person right-clicks the row for PR 918 titled "Retry failed payments" and picks Copy as Markdown
- **THEN** the clipboard holds `[Retry failed payments](https://github.com/acme/shop-api/pull/918)`

### Requirement: Notes under Links
Text items, typed notes and captured branch names, SHALL show in a "Notes" group under the
links, with its own small count, and SHALL hide the group while there are none. The Links head
SHALL count links only. The `+` on the head SHALL still take a link or a note.

#### Scenario: A branch name is captured
- **WHEN** terminal output names the branch `feature/PROJ-482-retry` in a side tab with two links
- **THEN** the Links head counts 2 and the Notes group shows the branch with a count of 1

### Requirement: Prompt rows show the name
A prompt row in the panel SHALL show only the prompt's name, wrapping to at most two lines,
with no placeholder names. The prompt tooltip SHALL keep the body with its placeholders.

#### Scenario: Long name
- **WHEN** the panel is 260 px wide and a prompt is named "Review the plan for the backend migration"
- **THEN** the whole name shows over two lines and no `{{...}}` text shows in the row
