//! Safe, interruptible, resumable scanner.
//!
//! Walks a single user-selected root directory. It never follows symlinks or
//! reparse points, refuses paths that resolve outside the root, applies an
//! extension allowlist, and honours a cancellation flag. Files that are
//! locked or disappear mid-scan are skipped rather than fatal.

use crate::path_policy::{
    canonicalize_root, extension_of, is_media_extension, is_safe_entry, sanitize_title,
};
use app_core::domain::MediaKind;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Stable identity derived from the canonical absolute path.
    pub id: String,
    pub path: String,
    pub size_bytes: Option<i64>,
    pub extension: Option<String>,
    pub title: String,
    pub hash: Option<String>,
    pub kind: MediaKind,
    /// Metadata read from a local sidecar, if one was present and usable.
    pub metadata: Option<app_core::domain::MetadataRecord>,
}

#[derive(Debug, Clone)]
pub struct ScanError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub files: Vec<ScannedFile>,
    pub errors: Vec<ScanError>,
    pub cancelled: bool,
}

pub struct Scanner {
    max_depth: usize,
    hash_limit_bytes: u64,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            max_depth: 64,
            hash_limit_bytes: 1024 * 1024 * 1024, // 1 GiB
        }
    }
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&self, root: &Path, cancel: &Arc<AtomicBool>) -> ScanOutcome {
        let mut outcome = ScanOutcome {
            files: Vec::new(),
            errors: Vec::new(),
            cancelled: false,
        };

        let canon_root = match canonicalize_root(root) {
            Ok(p) => p,
            Err(e) => {
                outcome.errors.push(ScanError {
                    path: root.display().to_string(),
                    message: e.to_string(),
                });
                return outcome;
            }
        };

        // Iterative DFS over directories, with a depth cap.
        let mut stack: Vec<(PathBuf, usize)> = vec![(canon_root.clone(), 0)];
        while let Some((dir, depth)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                outcome.cancelled = true;
                return outcome;
            }
            if depth > self.max_depth {
                continue;
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) => {
                    outcome.errors.push(ScanError {
                        path: dir.display().to_string(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            for entry in entries.flatten() {
                if cancel.load(Ordering::Relaxed) {
                    outcome.cancelled = true;
                    return outcome;
                }
                let path = entry.path();
                if !is_safe_entry(&canon_root, &path) {
                    // Symlink/reparse or escape: skip silently (counts as refused).
                    continue;
                }
                let meta = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if meta.is_dir() {
                    stack.push((path, depth + 1));
                } else if meta.is_file() {
                    let ext = extension_of(&path);
                    let allowed = ext.as_deref().map(is_media_extension).unwrap_or(false);
                    if !allowed {
                        continue;
                    }
                    // Allowlisted extensions that are not playable media
                    // (artwork) are skipped rather than reported as errors.
                    let Some(kind) = ext.as_deref().and_then(classify_kind) else {
                        continue;
                    };
                    let fm = crate::metadata::extract(&path);
                    let hash = if fm.size_bytes.unwrap_or(0) as u64 <= self.hash_limit_bytes {
                        hash_file(&path)
                    } else {
                        None
                    };
                    let path_string = path.to_string_lossy().to_string();
                    // Offline metadata only: a refused sidecar is reported
                    // against the file rather than silently ignored.
                    let metadata =
                        match media_metadata::NfoMetadataProvider::new().lookup_path(&path) {
                            Ok(record) => record,
                            Err(err) => {
                                outcome.errors.push(ScanError {
                                    path: path_string.clone(),
                                    message: err.to_string(),
                                });
                                None
                            }
                        };
                    outcome.files.push(ScannedFile {
                        id: stable_id(&path_string),
                        path: path_string,
                        size_bytes: fm.size_bytes,
                        extension: fm.extension.or(ext),
                        title: if fm.title.is_empty() {
                            sanitize_title(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))
                        } else {
                            fm.title
                        },
                        hash,
                        kind,
                        metadata,
                    });
                }
                // Other file types (sockets, etc.) are ignored.
            }
        }

        outcome
    }
}

fn hash_file(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Some(hex::encode(hasher.finalize()))
}

/// Stable identity for a media item: SHA-256 of the canonical absolute path.
pub fn stable_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hex::encode(hasher.finalize())
}

/// Map a lowercase extension to a media kind. Audio maps to `Movie` because the
/// schema `CHECK` only allows movie/series/episode.
pub fn classify_kind(extension: &str) -> Option<MediaKind> {
    match extension.to_ascii_lowercase().as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "m4v" | "mpg" | "mpeg" | "ts" | "mp3" | "flac"
        | "wav" | "aac" | "ogg" | "m4a" | "opus" | "wma" => Some(MediaKind::Movie),
        _ => None,
    }
}
