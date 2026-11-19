-- A link's name source (refine-context-links D4): 0 none, 1 text, 2 tool. The name sits in name.
ALTER TABLE context_items ADD COLUMN name_rank INTEGER NOT NULL DEFAULT 0;
