-- A window's own UI zoom in percent (task 4.1); NULL follows the Settings default zoom.
ALTER TABLE windows ADD COLUMN zoom_percent INTEGER;
