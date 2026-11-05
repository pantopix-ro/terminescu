-- Manual statuses on side tabs and tabs, and the order of saved statuses (D15, group 5).
ALTER TABLE side_tabs ADD COLUMN status_json TEXT;
ALTER TABLE tabs ADD COLUMN status_json TEXT;
ALTER TABLE statuses ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
