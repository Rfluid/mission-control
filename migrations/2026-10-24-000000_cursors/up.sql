-- Where each agent session's `wait` stopped, per mission: page events after `seq` are new to it.
CREATE TABLE cursors (
  mission TEXT NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  session TEXT NOT NULL,
  seq BIGINT NOT NULL,
  updated BIGINT NOT NULL,
  PRIMARY KEY (mission, session)
);
