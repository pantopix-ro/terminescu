-- Terminescu schema v1 (D18). Every table except owners carries owner_id; v1 has one local
-- owner. Session-only and secret variables never reach this file (D21). Times are Unix ms.

CREATE TABLE owners (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  is_local    INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0, 1)),
  created_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX owners_one_local ON owners (is_local) WHERE is_local = 1;

CREATE TABLE windows (
  id          TEXT PRIMARY KEY,
  owner_id    TEXT NOT NULL REFERENCES owners (id),
  position    INTEGER NOT NULL,
  frame_json  TEXT,
  created_at  INTEGER NOT NULL
);

CREATE TABLE side_tabs (
  id          TEXT PRIMARY KEY,
  owner_id    TEXT NOT NULL REFERENCES owners (id),
  window_id   TEXT NOT NULL REFERENCES windows (id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  title       TEXT,
  accent      TEXT,
  background  TEXT,
  created_at  INTEGER NOT NULL
);

-- layout_json holds the tab's split tree; its leaves name rows in panes.
CREATE TABLE tabs (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  position     INTEGER NOT NULL,
  title        TEXT,
  layout_json  TEXT NOT NULL,
  created_at   INTEGER NOT NULL
);

CREATE TABLE panes (
  id              TEXT PRIMARY KEY,
  owner_id        TEXT NOT NULL REFERENCES owners (id),
  tab_id          TEXT NOT NULL REFERENCES tabs (id) ON DELETE CASCADE,
  kind            TEXT NOT NULL CHECK (kind IN ('terminal', 'transcript', 'text')),
  daemon_pane_id  INTEGER,
  cwd             TEXT,
  document_id     TEXT,
  session_id      TEXT,
  created_at      INTEGER NOT NULL
);

-- Saved variables, links and text. Session-only and secret variables live in the daemon.
CREATE TABLE context_items (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('variable', 'link', 'text')),
  name         TEXT,
  value        TEXT NOT NULL,
  source       TEXT,
  position     INTEGER NOT NULL,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE UNIQUE INDEX context_variable_name
  ON context_items (side_tab_id, name) WHERE kind = 'variable';

-- A pin points at a transcript entry, optionally at a text quote inside it (D12).
CREATE TABLE pins (
  id            TEXT PRIMARY KEY,
  owner_id      TEXT NOT NULL REFERENCES owners (id),
  side_tab_id   TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  session_id    TEXT NOT NULL,
  entry_uuid    TEXT NOT NULL,
  quote_exact   TEXT,
  quote_prefix  TEXT,
  quote_suffix  TEXT,
  start_offset  INTEGER,
  end_offset    INTEGER,
  note          TEXT,
  created_at    INTEGER NOT NULL
);

CREATE TABLE prompts (
  id               TEXT PRIMARY KEY,
  owner_id         TEXT NOT NULL REFERENCES owners (id),
  key              TEXT NOT NULL,
  name             TEXT NOT NULL,
  tags_json        TEXT NOT NULL DEFAULT '[]',
  archived         INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
  current_version  INTEGER NOT NULL,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  UNIQUE (owner_id, key)
);

CREATE TABLE prompt_versions (
  id              TEXT PRIMARY KEY,
  owner_id        TEXT NOT NULL REFERENCES owners (id),
  prompt_id       TEXT NOT NULL REFERENCES prompts (id) ON DELETE CASCADE,
  version         INTEGER NOT NULL,
  body            TEXT NOT NULL,
  variables_json  TEXT NOT NULL DEFAULT '[]',
  created_at      INTEGER NOT NULL,
  UNIQUE (prompt_id, version)
);

-- side_tab_id NULL means a global command (D24).
CREATE TABLE commands (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT REFERENCES side_tabs (id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  body         TEXT NOT NULL,
  target       TEXT NOT NULL DEFAULT 'focused',
  launcher     INTEGER NOT NULL DEFAULT 0 CHECK (launcher IN (0, 1)),
  position     INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
);

-- side_tab_id NULL means a global status (D15).
CREATE TABLE statuses (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT REFERENCES side_tabs (id) ON DELETE CASCADE,
  label        TEXT NOT NULL,
  accent       TEXT,
  background   TEXT,
  created_at   INTEGER NOT NULL
);

CREATE TABLE status_rules (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  status_id    TEXT NOT NULL REFERENCES statuses (id) ON DELETE CASCADE,
  kind         TEXT NOT NULL,
  params_json  TEXT NOT NULL DEFAULT '{}',
  priority     INTEGER NOT NULL DEFAULT 0,
  enabled      INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))
);

CREATE TABLE extraction_rules (
  id             TEXT PRIMARY KEY,
  owner_id       TEXT NOT NULL REFERENCES owners (id),
  name           TEXT NOT NULL,
  pattern        TEXT NOT NULL,
  name_template  TEXT NOT NULL,
  builtin        INTEGER NOT NULL DEFAULT 0 CHECK (builtin IN (0, 1)),
  enabled        INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))
);

-- Claude sessions seen through hooks (D34). id is Claude's session id.
CREATE TABLE sessions (
  id               TEXT PRIMARY KEY,
  owner_id         TEXT NOT NULL REFERENCES owners (id),
  pane_id          TEXT REFERENCES panes (id) ON DELETE SET NULL,
  transcript_path  TEXT,
  cwd              TEXT,
  transcript_offset INTEGER NOT NULL DEFAULT 0,
  last_event       TEXT,
  started_at       INTEGER NOT NULL,
  ended_at         INTEGER
);

-- Text documents with hot exit (D28). Session-only documents never get a row.
CREATE TABLE documents (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT REFERENCES side_tabs (id) ON DELETE SET NULL,
  source_kind  TEXT NOT NULL CHECK (source_kind IN ('file', 'untitled', 'variable', 'prompt_version', 'editor_request')),
  source_ref   TEXT,
  title        TEXT NOT NULL,
  content      TEXT,
  dirty        INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
  updated_at   INTEGER NOT NULL
);

-- Files Claude wrote or edited, per side tab (D30).
CREATE TABLE context_files (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  path         TEXT NOT NULL,
  change       TEXT NOT NULL CHECK (change IN ('added', 'modified')),
  session_id   TEXT,
  updated_at   INTEGER NOT NULL,
  UNIQUE (side_tab_id, path)
);
