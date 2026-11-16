CREATE TABLE IF NOT EXISTS media_items (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('movie','series','episode')),
  title TEXT NOT NULL,
  year INTEGER,
  synopsis TEXT,
  runtime_minutes INTEGER,
  genres TEXT NOT NULL DEFAULT '[]',
  availability_state TEXT NOT NULL DEFAULT 'available',
  availability_last_seen TEXT,
  progress_position REAL,
  progress_duration REAL,
  progress_updated_at TEXT,
  progress_completed INTEGER NOT NULL DEFAULT 0,
  director TEXT,
  -- `cast` is a SQL keyword, so the column carries a suffix.
  cast_members TEXT NOT NULL DEFAULT '[]',
  series_id TEXT,
  season_number INTEGER,
  episode_number INTEGER,
  next_up INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media_files (
  id TEXT PRIMARY KEY,
  media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
  size_bytes INTEGER,
  hash TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS library_roots (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  item_count INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'ok',
  last_scan_at TEXT,
  error_message TEXT
);

CREATE TABLE IF NOT EXISTS watchlist_items (
  id TEXT PRIMARY KEY,
  media_id TEXT NOT NULL UNIQUE REFERENCES media_items(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  added_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS queue_items (
  id TEXT PRIMARY KEY,
  media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending'
);

CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  data TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_items_kind ON media_items(kind);
CREATE INDEX IF NOT EXISTS idx_media_items_title ON media_items(title);
CREATE INDEX IF NOT EXISTS idx_media_items_series ON media_items(series_id);