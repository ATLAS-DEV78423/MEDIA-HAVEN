//! Basic metadata extraction from the filesystem.
//!
//! Full duration/codec metadata requires ffprobe, which is an optional
//! runtime dependency and is not available on every host. Until that lands,
//! we record what the OS can tell us for free: size, extension, and a title
//! derived from the filename. These fields are sufficient for browsing and
//! search; technical metadata is left as `None`.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size_bytes: Option<i64>,
    pub extension: Option<String>,
    pub title: String,
}

pub fn extract(path: &Path) -> FileMetadata {
    let size_bytes = std::fs::metadata(path).ok().map(|m| m.len() as i64);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let title = crate::path_policy::sanitize_title(stem);
    FileMetadata {
        size_bytes,
        extension,
        title,
    }
}
