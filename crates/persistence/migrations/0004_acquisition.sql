CREATE TABLE torrent_tasks (
  hash TEXT PRIMARY KEY,
  magnet_or_source TEXT NOT NULL,
  state TEXT NOT NULL,          -- queued|downloading|ready|error|removing
  progress REAL NOT NULL DEFAULT 0,
  save_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE download_items (
  id TEXT PRIMARY KEY,
  torrent_hash TEXT NOT NULL REFERENCES torrent_tasks(hash),
  media_id TEXT,                -- resolved once identified; NULL while downloading
  file_path TEXT NOT NULL,      -- container path, /downloads/...
  state TEXT NOT NULL,          -- downloading|ready|playing|watched|cleanup_pending
  created_at TEXT NOT NULL
);
CREATE TABLE file_mappings (
  file_path TEXT PRIMARY KEY,
  torrent_hash TEXT NOT NULL REFERENCES torrent_tasks(hash),
  media_id TEXT,                -- supports multi-episode torrents (spec §10)
  episode_no INTEGER
);
CREATE TABLE prefetch_entries (
  media_id TEXT PRIMARY KEY,
  requested_for TEXT NOT NULL,  -- the currently playing media id
  priority INTEGER NOT NULL,
  state TEXT NOT NULL
);
CREATE TABLE cleanup_entries (
  media_id TEXT PRIMARY KEY,
  eligible_at TEXT NOT NULL,
  state TEXT NOT NULL,          -- pending|confirmed|removed|failed
  outcome TEXT
);
CREATE INDEX idx_file_mappings_media ON file_mappings(media_id);
