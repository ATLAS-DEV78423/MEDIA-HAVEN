-- Append-only history for queue entries that leave the active queue.
-- Rows are only ever inserted (on explicit remove); user media files are never
-- touched. Additive and reversible by dropping this table.
CREATE TABLE IF NOT EXISTS queue_history (
  id INTEGER PRIMARY KEY,
  queue_id TEXT NOT NULL,
  media_id TEXT NOT NULL,
  title TEXT,
  state TEXT NOT NULL,
  position INTEGER NOT NULL,
  recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_queue_history_media ON queue_history(media_id);
