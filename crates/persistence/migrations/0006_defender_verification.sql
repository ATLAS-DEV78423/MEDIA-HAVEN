-- Per-download Windows Defender gate (security hardening, Task 2).
-- Default 'not_scanned' means every existing and newly inserted download stays
-- unplayable until a Defender result is recorded: fail closed by construction.
ALTER TABLE download_items ADD COLUMN defender_state TEXT NOT NULL DEFAULT 'not_scanned';
ALTER TABLE download_items ADD COLUMN defender_detail TEXT;
CREATE INDEX idx_download_items_defender_state ON download_items(defender_state);
