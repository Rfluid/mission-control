CREATE TABLE missions (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  session TEXT,
  project_dir TEXT,
  token TEXT NOT NULL,
  created BIGINT NOT NULL,
  archived BOOLEAN NOT NULL DEFAULT 0
);

-- Every collection (packages, prs, needs, ...) is rows here. `body` is a JSON object.
CREATE TABLE records (
  mission TEXT NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  collection TEXT NOT NULL,
  key TEXT NOT NULL,
  state TEXT,
  body TEXT NOT NULL,
  updated BIGINT NOT NULL,
  PRIMARY KEY (mission, collection, key)
);

-- Append-only: activity log, answers, button clicks, customize requests, config changes.
CREATE TABLE events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  mission TEXT NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  text TEXT NOT NULL,
  data TEXT,
  at BIGINT NOT NULL
);
CREATE INDEX events_mission ON events (mission, seq);

-- The mission config layer, one row per version (latest wins; older rows are undo history).
CREATE TABLE mission_config (
  mission TEXT NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  doc TEXT NOT NULL,
  note TEXT,
  at BIGINT NOT NULL,
  PRIMARY KEY (mission, version)
);
