//! Size-capped, 30-day filesystem cache for metadata payloads.
//!
//! Stores opaque bytes keyed by a caller-supplied string. Freshness comes from
//! file mtime; eviction removes the oldest files until the directory fits the
//! byte cap. No network, no logging of keys (they may contain search terms).

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Metadata older than this is refetched rather than served.
pub const CACHE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
    max_bytes: u64,
}

impl Cache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self {
        Self { dir, max_bytes }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Fresh bytes for `key`, or `None` on a miss, stale entry or read failure.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.key_path(key);
        let meta = fs::metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let modified = meta.modified().ok()?;
        if SystemTime::now()
            .duration_since(modified)
            .unwrap_or(CACHE_TTL)
            >= CACHE_TTL
        {
            return None;
        }
        fs::read(&path).ok()
    }

    /// Write `bytes` for `key`, then evict the oldest files if over the cap.
    pub fn put(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::write(self.key_path(key), bytes)?;
        self.enforce_cap()
    }

    fn key_path(&self, key: &str) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        self.dir.join(format!("{:016x}.json", hasher.finish()))
    }

    fn enforce_cap(&self) -> io::Result<()> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if !meta.is_file() {
                continue;
            }
            let modified = meta.modified().unwrap_or(UNIX_EPOCH);
            entries.push((modified, entry.path(), meta.len()));
        }

        let total: u64 = entries.iter().map(|entry| entry.2).sum();
        if total <= self.max_bytes {
            return Ok(());
        }

        let mut excess = total - self.max_bytes;
        entries.sort_by_key(|entry| entry.0); // oldest first
        for (_, path, len) in entries {
            if excess == 0 {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                excess = excess.saturating_sub(len);
            }
        }
        Ok(())
    }
}
