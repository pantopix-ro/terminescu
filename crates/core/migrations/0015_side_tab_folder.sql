-- A folder made from the sidebar's + stays a folder with one tab or none (task 8.91).
ALTER TABLE side_tabs ADD COLUMN folder INTEGER NOT NULL DEFAULT 0;
