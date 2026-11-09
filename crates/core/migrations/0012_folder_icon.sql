-- A side tab with two tabs or more is now a folder (task 8.78); its icon follows the name.
UPDATE side_tabs SET icon = 'folder' WHERE icon = 'group';
UPDATE tabs SET icon = 'folder' WHERE icon = 'group';
