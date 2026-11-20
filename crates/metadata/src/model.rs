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
/// Certification strings are short ("PG-13", "R"); anything longer is hostile.
pub(crate) const MAX_AGE_RATING_CHARS: usize = 32;

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

/// One IMDb listing, merged from `title.basics` + `title.ratings`.
///
/// Display-only. `age_rating` is filled by the opt-in OMDb seam (Task 1); the
/// official IMDb datasets carry no certification, so it stays `None` here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImdbRecord {
    pub imdb_id: String,
    pub title: String,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub age_rating: Option<String>,
}

/// Why an IMDb dataset could not be used. Every variant refuses the dataset;
/// the caller degrades to local data rather than showing a partial record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImdbError {
    #[error("imdb dataset is not valid UTF-8")]
    NotUtf8,
    #[error("imdb dataset could not be read: {0}")]
    Unreadable(String),
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
