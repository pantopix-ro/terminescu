## ADDED Requirements

### Requirement: Section heads
The context panel SHALL show Variables, Links, Files, Pins and Prompts as peer
sections, then the plugin sections. Each section SHALL have one head row with its title, its
count, and a `+` button when the section has an add action (Variables, Links,
Prompts). A section with no rows SHALL show its head only, with no empty-state text; Pins
SHALL hide entirely while empty. An error SHALL still show under its head.

#### Scenario: Empty side tab
- **WHEN** a new side tab with no variables, links, files or pins shows the pinned panel
- **THEN** the Variables, Links and Files heads each show 0 with no sentence under them, and Pins does not show

#### Scenario: Add a link from the head
- **WHEN** the person clicks `+` on the Links head, types `https://example.test/a` and presses Enter
- **THEN** the add box closes and the section counts 1

#### Scenario: Plugin count
- **WHEN** the git section finds 3 changed files
- **THEN** its head shows the title and 3, and with no changes it shows the title and 0 with nothing under it

### Requirement: Compact variables
Each variable SHALL take one line with its name and its value cut to fit. Its actions SHALL
show on hover or keyboard focus. Hovering the name or value SHALL show a tooltip with the
value, its storage mode and expiry, and its source. A secret's value SHALL never show.

#### Scenario: Details on hover
- **WHEN** the person rests the pointer on `{{from_date}}`, a session variable with an hour left
- **THEN** the row stays one line, and a tooltip shows `2026-11-01` and "gone on reboot · 1h left"

#### Scenario: Secret stays hidden
- **WHEN** the person hovers `{{API_TOKEN}}`
- **THEN** the tooltip says it inserts as `$API_TOKEN` and holds no part of the value

### Requirement: Prompt clicks
A left click on a prompt in the panel SHALL insert the prompt, filled from the side tab's
context, into the focused pane as a paste without Enter. If a placeholder has no value, the
fill-in dialog SHALL open on that prompt instead. A right click SHALL open the prompt in the
library editor. The Prompts head `+` SHALL open the library on a new prompt.

#### Scenario: All values known
- **WHEN** the side tab has `ticket` and the person left-clicks `review-plan`, which uses `{{ticket}}`
- **THEN** the filled text sits at the terminal prompt, no Enter is sent and no dialog opens

#### Scenario: A value missing
- **WHEN** the person left-clicks `review-plan` in a side tab without `ticket`
- **THEN** the fill-in dialog opens on `review-plan` with the `ticket` field focused

#### Scenario: Escape from a panel click
- **WHEN** the dialog opened from a panel click and the person presses Escape
- **THEN** the dialog closes, without showing the list of prompts

#### Scenario: Edit by right click
- **WHEN** the person right-clicks `review-plan` in the panel
- **THEN** the library opens with `review-plan` selected and no Edit menu shows

### Requirement: Fill-in keys
In the fill-in dialog, Enter in a value field SHALL insert the prompt without Enter, and an
empty value SHALL stay as `{{name}}`. Tab and Shift-Tab SHALL move between the value fields
only, wrapping at the ends.

#### Scenario: Insert with a value left empty
- **WHEN** a prompt asks for `ticket` and `branch`, the person types `PROJ-482` for `ticket` and presses Enter in that field
- **THEN** the text goes in with `PROJ-482` and `{{branch}}`, and the dialog closes

#### Scenario: Tab between values
- **WHEN** the `branch` field, the last one, has focus and the person presses Tab
- **THEN** the `ticket` field gets focus

### Requirement: Prompt tooltip
Hovering a prompt in the panel SHALL show its whole body in a tooltip. Each placeholder with a
value SHALL switch once a second between `{{name}}` and the value, both in the placeholder
colour, and the tooltip SHALL keep its size in both states. A secret SHALL show as `$NAME`.
With reduced motion set, the values SHALL show without switching.

#### Scenario: Blink at a fixed size
- **WHEN** the person hovers `review-plan` and `ticket` is `PROJ-482`
- **THEN** the tooltip shows `{{ticket}}` and then `PROJ-482` in its place, in the same colour, and its width and height do not change

#### Scenario: Long value
- **WHEN** `branch` holds a 40-character name
- **THEN** its slot is 24 characters wide and the value ends in `…`

#### Scenario: Name longer than the cap
- **WHEN** the prompt uses a 30-character placeholder name and its value is 40 characters
- **THEN** the slot is 30 characters wide and the value ends in `…`

#### Scenario: Missing value
- **WHEN** the prompt uses `{{owner}}` and the side tab has no `owner`
- **THEN** `{{owner}}` shows in both states and does not blink

### Requirement: Unique links and text
A side tab SHALL show each link and each text once, also after a merge or a moved tab brings a
copy. Removing one SHALL remove its copies. A link SHALL keep its end in view and cut its middle.

#### Scenario: Same link twice
- **WHEN** `https://example.test/acme/platform-api/pull/4821` is added twice to one side tab
- **THEN** the panel shows it once and counts 1

#### Scenario: Links on one host
- **WHEN** the side tab holds `.../pull/4821` and `.../pull/4822` on the same host
- **THEN** both rows show their different endings in a narrow panel
