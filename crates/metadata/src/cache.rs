//! Size-capped, 30-day filesystem cache for metadata payloads.
//!
//! Stores opaque bytes keyed by a caller-supplied string. Freshness comes from
//! file mtime; eviction removes the oldest files until the directory fits the
//! byte cap. No network, no logging of keys (they may contain search terms).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
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

/// Age-rating cache size cap. Certifications are a few bytes; 1 MiB is ample
/// and keeps a hostile dataset from growing the cache without bound.
pub const DEFAULT_IMDB_CACHE_MAX_BYTES: u64 = 1024 * 1024;

/// In-memory twin of the SQLite `imdb_cache` table, used by the OMDb provider.
/// Same contract: 30-day TTL, byte cap, oldest-first eviction. No network, no
/// file I/O; the durable adapter lives in `media-persistence`.
#[derive(Debug)]
pub struct RatingCache {
    entries: HashMap<String, (String, SystemTime)>,
    max_bytes: u64,
}

impl RatingCache {
    pub fn new(max_bytes: u64) -> Self {
        Self {
            entries: HashMap::new(),
            max_bytes,
        }
    }

    /// Fresh payload for `id`, or `None` on a miss or stale entry.
    pub fn get(&self, id: &str) -> Option<String> {
        let (payload, stored_at) = self.entries.get(id)?;
        if stored_at.elapsed().unwrap_or(CACHE_TTL) >= CACHE_TTL {
            return None;
        }
        Some(payload.clone())
    }

    pub fn put(&mut self, id: &str, payload: &str) {
        self.put_at(id, payload, SystemTime::now());
    }

    /// Seeding seam for restoring a durable entry and for freshness tests.
    pub fn put_at(&mut self, id: &str, payload: &str, stored_at: SystemTime) {
        self.entries
            .insert(id.to_string(), (payload.to_string(), stored_at));
        self.enforce_cap();
    }

    fn enforce_cap(&mut self) {
        let total: u64 = self.entries.values().map(|(p, _)| p.len() as u64).sum();
        if total <= self.max_bytes {
            return;
        }
        let mut excess = total - self.max_bytes;
        let mut oldest: Vec<(String, SystemTime)> = self
            .entries
            .iter()
            .map(|(id, (_, at))| (id.clone(), *at))
            .collect();
        oldest.sort_by_key(|(_, at)| *at);
        for (id, _) in oldest {
            if excess == 0 {
                break;
            }
            if let Some((payload, _)) = self.entries.remove(&id) {
                excess = excess.saturating_sub(payload.len() as u64);
            }
        }
    }
}
