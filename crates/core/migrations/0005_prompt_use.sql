-- The library sorts by last use and shows a use count (D14, task 6.3).
ALTER TABLE prompts ADD COLUMN last_used_at INTEGER;
ALTER TABLE prompts ADD COLUMN use_count INTEGER NOT NULL DEFAULT 0;
CREATE INDEX prompt_versions_by_prompt ON prompt_versions (prompt_id, version);
