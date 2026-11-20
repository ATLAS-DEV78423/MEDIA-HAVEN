-- IMDb/OMDb age-rating cache (IMDb plan Task 1). Rows exist only after the
-- operator opts in; no API key, path or content source is stored here.
CREATE TABLE imdb_cache (
  id TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  source TEXT NOT NULL
);

CREATE INDEX imdb_cache_fetched_at_idx ON imdb_cache(fetched_at);
