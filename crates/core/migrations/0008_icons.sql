-- An icon picked from the fixed list (task 8.16); NULL shows the default icon.
ALTER TABLE side_tabs ADD COLUMN icon TEXT;
ALTER TABLE tabs ADD COLUMN icon TEXT;
