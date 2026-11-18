-- Track where a library root lives on disk so it can be rescanned and removed
-- without touching source files.
ALTER TABLE library_roots ADD COLUMN path TEXT;

-- Track which root a media item came from, and where the source file lives.
ALTER TABLE media_items ADD COLUMN root_id TEXT REFERENCES library_roots(id) ON DELETE SET NULL;
ALTER TABLE media_files ADD COLUMN path TEXT;

CREATE INDEX IF NOT EXISTS idx_media_items_root ON media_items(root_id);