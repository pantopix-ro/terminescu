-- Small app-wide values, such as the daemon instance the layout was saved against (task 3.3).
CREATE TABLE app_state (
  owner_id  TEXT NOT NULL REFERENCES owners (id),
  key       TEXT NOT NULL,
  value     TEXT NOT NULL,
  PRIMARY KEY (owner_id, key)
);
