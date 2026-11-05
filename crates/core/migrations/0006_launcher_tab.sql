-- The tab a launcher command runs in (D24); NULL until it first runs or after its tab closed.
ALTER TABLE commands ADD COLUMN tab_id TEXT;
