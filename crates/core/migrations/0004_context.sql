-- Context items record the tab that made them, and pins can come from a terminal pane (D12,
-- group 6). A terminal pin has no session or entry; it keeps its pane and buffer rows instead.
ALTER TABLE context_items ADD COLUMN origin_tab_id TEXT;

CREATE TABLE pins_v2 (
  id             TEXT PRIMARY KEY,
  owner_id       TEXT NOT NULL REFERENCES owners (id),
  side_tab_id    TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  origin_tab_id  TEXT,
  session_id     TEXT,
  entry_uuid     TEXT,
  pane_id        TEXT,
  row_start      INTEGER,
  row_end        INTEGER,
  quote_exact    TEXT,
  quote_prefix   TEXT,
  quote_suffix   TEXT,
  start_offset   INTEGER,
  end_offset     INTEGER,
  note           TEXT,
  created_at     INTEGER NOT NULL,
  CHECK ((session_id IS NOT NULL AND entry_uuid IS NOT NULL) OR pane_id IS NOT NULL)
);
INSERT INTO pins_v2 (id, owner_id, side_tab_id, session_id, entry_uuid, quote_exact, quote_prefix,
  quote_suffix, start_offset, end_offset, note, created_at)
SELECT id, owner_id, side_tab_id, session_id, entry_uuid, quote_exact, quote_prefix, quote_suffix,
  start_offset, end_offset, note, created_at FROM pins;
DROP TABLE pins;
ALTER TABLE pins_v2 RENAME TO pins;
