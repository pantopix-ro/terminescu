-- The animal or bird a side tab is named after once it holds two tabs (task 8.60). Tabs no
-- longer get one; their old `tabs.name` column stays, unread.
ALTER TABLE side_tabs ADD COLUMN name TEXT;
-- A row keeps the name its first tab had, so a group that was "Otter" stays "Otter".
UPDATE side_tabs SET name = (
  SELECT t.name FROM tabs t WHERE t.side_tab_id = side_tabs.id AND t.owner_id = side_tabs.owner_id
  ORDER BY t.position LIMIT 1
);
