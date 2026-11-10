-- Plugins (add-plugin-architecture D10). Settings hold the on/off switch and each plugin's
-- own settings; row data holds per-row facts that die with the row.
CREATE TABLE plugin_settings (
  owner_id    TEXT NOT NULL REFERENCES owners (id),
  plugin_id   TEXT NOT NULL,
  key         TEXT NOT NULL,
  value_json  TEXT NOT NULL,
  PRIMARY KEY (owner_id, plugin_id, key)
);

CREATE TABLE plugin_side_tab_data (
  owner_id     TEXT NOT NULL REFERENCES owners (id),
  side_tab_id  TEXT NOT NULL REFERENCES side_tabs (id) ON DELETE CASCADE,
  plugin_id    TEXT NOT NULL,
  key          TEXT NOT NULL,
  value_json   TEXT NOT NULL,
  PRIMARY KEY (side_tab_id, plugin_id, key)
);
