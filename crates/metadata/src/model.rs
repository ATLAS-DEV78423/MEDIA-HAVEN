//! Parsed sidecar data, the refusal cases, and the caps applied to values.

use app_core::domain::MetadataRecord;
use std::path::Path;

/// A sidecar bigger than this is refused rather than parsed. Kodi NFO files
/// are a few kilobytes; 256 KiB leaves room for long plot summaries.
pub const MAX_NFO_BYTES: u64 = 256 * 1024;

pub(crate) const MAX_TITLE_CHARS: usize = 512;
pub(crate) const MAX_SYNOPSIS_CHARS: usize = 4000;
pub(crate) const MAX_GENRE_CHARS: usize = 64;
pub(crate) const MAX_GENRES: usize = 32;

/// Why a sidecar was not used. Every variant is a refusal, not a fallback the
/// caller may ignore silently: the scanner reports it against the file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NfoError {
    #[error("nfo file exceeds the 256 KiB limit")]
    TooLarge,
    #[error("nfo declares a DTD or entity and is refused")]
    DoctypeRefused,
    #[error("nfo is not valid UTF-8")]
    NotUtf8,
    #[error("nfo contains control characters")]
    ControlCharacters,
    #[error("nfo is malformed: {0}")]
    Malformed(String),
    #[error("nfo could not be read: {0}")]
    Unreadable(String),
}

/// What an `.nfo` file said. Empty elements stay `None` so that a sparse file
/// never overwrites data the library already has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NfoDocument {
    pub title: Option<String>,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub genres: Vec<String>,
}

impl NfoDocument {
    pub fn into_record(self) -> MetadataRecord {
        MetadataRecord {
            title: self.title,
            year: self.year,
            synopsis: self.synopsis,
            runtime_minutes: self.runtime_minutes,
            genres: self.genres,
        }
    }
}

/// Paths checked for a sidecar, most specific first.
pub fn candidates(media: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(stem) = media.file_stem().and_then(|s| s.to_str()) {
        out.push(media.with_file_name(format!("{stem}.nfo")));
    }
    if let Some(dir) = media.parent() {
        out.push(dir.join("movie.nfo"));
    }
    out
}
