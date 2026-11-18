//! Path safety policy for the scanner.
//!
//! Every path that crosses the trust boundary (a user-selected root) is
//! canonicalised and containment-checked before any read. Symlinks and
//! reparse points are refused, and traversal attempts that resolve outside
//! the root are rejected with an error rather than silently skipped.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    NotADirectory,
    Unreadable(String),
    EscapesRoot,
    SymlinkOrReparse,
    InvalidUtf8,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::NotADirectory => write!(f, "path is not a directory"),
            PathError::Unreadable(m) => write!(f, "path is unreadable: {m}"),
            PathError::EscapesRoot => write!(f, "path resolves outside the root"),
            PathError::SymlinkOrReparse => write!(f, "symlink or reparse point refused"),
            PathError::InvalidUtf8 => write!(f, "path is not valid UTF-8"),
        }
    }
}

impl std::error::Error for PathError {}

/// Canonicalise a candidate root directory. Returns an error if it is not a
/// directory, unreadable, or not valid UTF-8.
pub fn canonicalize_root(root: &Path) -> Result<PathBuf, PathError> {
    let meta = std::fs::symlink_metadata(root).map_err(|e| PathError::Unreadable(e.to_string()))?;
    if meta.file_type().is_symlink() {
        return Err(PathError::SymlinkOrReparse);
    }
    if !meta.is_dir() {
        return Err(PathError::NotADirectory);
    }
    let canon = root
        .canonicalize()
        .map_err(|e| PathError::Unreadable(e.to_string()))?;
    if canon.to_str().is_none() {
        return Err(PathError::InvalidUtf8);
    }
    Ok(canon)
}

/// True if `entry` resolves to a path strictly inside `root` (canonical).
/// Symlinks and reparse points are treated as unsafe and return false.
pub fn is_safe_entry(root: &Path, entry: &Path) -> bool {
    let meta = match std::fs::symlink_metadata(entry) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if meta.file_type().is_symlink() {
        return false;
    }
    let canon_entry = match entry.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let canon_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    canon_entry.starts_with(&canon_root) && canon_entry != canon_root
}

/// True if `candidate` (already canonical or not) stays within `root`.
pub fn within(root: &Path, candidate: &Path) -> bool {
    let (Ok(cr), Ok(ce)) = (root.canonicalize(), candidate.canonicalize()) else {
        return false;
    };
    ce.starts_with(&cr)
}

/// Allowlisted media extensions (lowercase, without leading dot).
pub fn is_media_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp4"
            | "mkv"
            | "avi"
            | "mov"
            | "webm"
            | "m4v"
            | "mpg"
            | "mpeg"
            | "ts"
            | "mp3"
            | "flac"
            | "wav"
            | "aac"
            | "ogg"
            | "m4a"
            | "opus"
            | "wma"
            | "jpg"
            | "jpeg"
            | "png"
            | "webp"
            | "gif"
            | "bmp"
    )
}

/// Extract a lowercase extension without the dot, if present.
pub fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

/// Sanitise a file stem into a display title: drop control characters, trim,
/// truncate to a sane length, and fall back to the filename if empty.
pub fn sanitize_title(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .filter(|c| !c.is_control() && *c != '\u{202e}')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else if trimmed.chars().count() > 200 {
        trimmed.chars().take(200).collect()
    } else {
        trimmed.to_string()
    }
}
