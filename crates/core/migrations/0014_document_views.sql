-- A document a plugin pane view draws, such as a diff tab: its view kind and source as JSON
-- (harden-plugin-host D6). Null draws plain text.
ALTER TABLE documents ADD COLUMN view_json TEXT;
