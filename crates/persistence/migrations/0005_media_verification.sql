-- Per-download verification gate (security hardening, Task 1).
-- Default 'pending' means every existing and newly inserted download starts
-- unplayable until the probe result is recorded: fail closed by construction.
ALTER TABLE download_items ADD COLUMN verification_state TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE download_items ADD COLUMN verification_detail TEXT;
CREATE INDEX idx_download_items_verification_state ON download_items(verification_state);
