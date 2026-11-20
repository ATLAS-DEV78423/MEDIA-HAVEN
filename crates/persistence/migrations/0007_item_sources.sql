-- R-01 per-item operator-supplied source mapping (ADR-0021).
-- No bundled sources: a row exists only when the operator supplied a source.
CREATE TABLE item_sources (
  media_id TEXT PRIMARY KEY,
  source TEXT NOT NULL,
  source_name TEXT
);
