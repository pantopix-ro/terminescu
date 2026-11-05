-- Text documents (D28): the disk state a buffer started from, so a start can tell that a file
-- changed while the app was closed. Files Claude edits (D30) keep their tab and the cwd that
-- makes their path short.
ALTER TABLE documents ADD COLUMN base_sig TEXT;
ALTER TABLE context_files ADD COLUMN origin_tab_id TEXT;
ALTER TABLE context_files ADD COLUMN cwd TEXT;
