# Spec Delta

## ADDED Requirements

### Requirement: Versioned prompts
The system SHALL store each prompt under a unique key as immutable versions of a Markdown body
with `{{variable}}` placeholders and images stored outside the body. Saving SHALL create a
version; older versions SHALL stay readable and restorable.

#### Scenario: Restore an older version
- **WHEN** the person restores version 2 of a prompt at version 5
- **THEN** version 6 is created with the body of version 2

### Requirement: Prompt library
The system SHALL provide a library listing every prompt with name, tags, last used and use
count, searchable by name, tag and body, with duplicate, archive and version history. Prompts
SHALL be edited in the same text editor as text tabs, and saving SHALL create a new version.

#### Scenario: Find by body text
- **WHEN** the person searches the library for `confidence`
- **THEN** every prompt whose current body contains the word is listed

#### Scenario: Add a prompt from the empty space
- **WHEN** the pointer rests on the empty library space under the last prompt
- **THEN** a `+` shows there, as in the sidebar, and a click on it opens the new prompt form

#### Scenario: Edit a prompt in a text tab
- **WHEN** the person clicks `review-plan` in the context panel, edits it and presses `⌘S`
- **THEN** the prompt gets version 5 and version 4 stays restorable

### Requirement: Insert with variables
`⌘I` SHALL open a palette of prompts and context variables. Inserting a prompt SHALL resolve
`{{name}}` from the side tab's context and `{{pins}}` from its pins, SHALL ask inline for
missing variables, SHALL allow editing before insert, and SHALL write the text to the focused
pane as a bracketed paste without Enter unless the person picks "insert and send".

#### Scenario: Missing variable
- **WHEN** the person inserts `review-plan`, which uses `{{ticket}}`, in a side tab without `ticket`
- **THEN** the dialog asks for `ticket`, and the value can be saved to the context

#### Scenario: Nothing sent by itself
- **WHEN** a prompt is inserted into a Claude pane with plain Insert
- **THEN** the text sits in Claude's input and no turn starts

### Requirement: Save as prompt
The system SHALL save the edited text in the insert dialog, or any message the person sent in a
transcript pane, as a new prompt, and SHALL offer to replace text equal to a context
variable's value with that `{{name}}`.

#### Scenario: Save a message
- **WHEN** the person saves a sent message containing the value of `branch` as a prompt
- **THEN** the save offers `{{branch}}` in place of the branch name and the prompt appears in the library
