//! Optional IMDb metadata provider — OFF by default, no network in this crate.
//!
//! All data comes from dataset files the operator supplies; nothing is bundled,
//! scraped or fetched here. The fetch is a seam (`ImdbFetch`) so every path is
//! testable in memory. With opt-in off, no fetch is called at all. This adds to
//! ADR-0011 (TMDB primary, local NFO fallback); it does not replace it.

use crate::model::{
    ImdbError, ImdbRecord, MAX_AGE_RATING_CHARS, MAX_GENRES, MAX_GENRE_CHARS, MAX_TITLE_CHARS,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Rendered wherever IMDb-sourced data appears (plan Task 2).
pub const IMDB_ATTRIBUTION: &str = "Ratings and age ratings: IMDb / OMDb";

/// Only movie/show listings belong in a media library; episodes, video games
/// and other `titleType` values are skipped as noise.
const LISTING_TYPES: &[&str] = &["movie", "tvMovie", "tvSeries", "tvMiniSeries"];

/// Pluggable dataset source. No implementation here performs I/O; the default
/// provider fetch fails closed. The app injects a real implementation only
/// after the operator opts in.
pub trait ImdbFetch: Send + Sync {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError>;
    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError>;
    /// Unknown ids return `Ok(None)`.
    fn by_id(&self, imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError>;
}

/// Fail-closed default: no transport and no file reads until configured.
#[derive(Debug, Default)]
struct UnconfiguredFetch;

impl ImdbFetch for UnconfiguredFetch {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        Err(ImdbError::Unreadable(
            "IMDb fetch is not configured".to_string(),
        ))
    }
    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        Err(ImdbError::Unreadable(
            "IMDb fetch is not configured".to_string(),
        ))
    }
    fn by_id(&self, _imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError> {
        Err(ImdbError::Unreadable(
            "IMDb fetch is not configured".to_string(),
        ))
    }
}

/// Master switch. `enabled: false` (the derived default) makes every provider
/// call a zero-fetch no-op.
#[derive(Debug, Clone, Default)]
pub struct ImdbOptions {
    pub enabled: bool,
}

pub struct ImdbProvider {
    options: ImdbOptions,
    fetch: Box<dyn ImdbFetch>,
}

impl ImdbProvider {
    pub fn new(options: ImdbOptions) -> Self {
        Self::with_fetch(options, Box::new(UnconfiguredFetch))
    }

    pub fn with_fetch(options: ImdbOptions, fetch: Box<dyn ImdbFetch>) -> Self {
        Self { options, fetch }
    }

    pub fn options(&self) -> &ImdbOptions {
        &self.options
    }

    pub fn attribution(&self) -> &'static str {
        IMDB_ATTRIBUTION
    }

    /// All listings the operator's datasets provide, or `None` when disabled
    /// or when either dataset fails. Failures never reach the caller as errors.
    pub fn records(&self) -> Option<Vec<ImdbRecord>> {
        if !self.options.enabled {
            return None;
        }
        let basics = self.fetch.basics().ok()?;
        let ratings = self.fetch.ratings().ok()?;
        Some(merge(basics, ratings))
    }

    /// One listing by IMDb id, or `None` when disabled, unknown or failed.
    pub fn by_id(&self, imdb_id: &str) -> Option<ImdbRecord> {
        if !self.options.enabled {
            return None;
        }
        self.fetch.by_id(imdb_id).ok().flatten()
    }
}

/// In-memory `ImdbFetch` for tests and downstream seams. Call count lets tests
/// prove that a disabled provider performs no fetch ("no network called").
#[derive(Debug, Default)]
pub struct MemoryFetch {
    pub basics: Vec<ImdbRecord>,
    pub ratings: Vec<ImdbRecord>,
    calls: Arc<AtomicUsize>,
}

impl MemoryFetch {
    pub fn new(basics: Vec<ImdbRecord>, ratings: Vec<ImdbRecord>) -> Self {
        Self {
            basics,
            ratings,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ImdbFetch for MemoryFetch {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.basics.clone())
    }
    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.ratings.clone())
    }
    fn by_id(&self, imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.basics.iter().find(|r| r.imdb_id == imdb_id).cloned())
    }
}

/// Shared handle so a test can inspect calls after moving the fetch into a
/// provider.
impl ImdbFetch for Arc<MemoryFetch> {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        (**self).basics()
    }
    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        (**self).ratings()
    }
    fn by_id(&self, imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError> {
        (**self).by_id(imdb_id)
    }
}

/// Reads the operator-supplied official IMDb TSV files from a host directory.
/// Nothing is bundled, scraped or fetched here. A disabled `ImdbProvider` never
/// calls this, so an unconfigured directory is never even opened. The read
/// counter lets tests prove the opt-in gate performs zero file reads.
#[derive(Debug)]
pub struct FileImdbFetch {
    pub dir: PathBuf,
    reads: Arc<AtomicUsize>,
}

impl FileImdbFetch {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            reads: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    fn read(&self, name: &str) -> Result<String, ImdbError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let bytes = std::fs::read(self.dir.join(name))
            .map_err(|error| ImdbError::Unreadable(error.to_string()))?;
        String::from_utf8(bytes).map_err(|_| ImdbError::NotUtf8)
    }

    /// Optional `age_ratings.tsv` (`tconst<TAB>certification`). Absent or bad
    /// rows degrade to no certification — the official datasets carry none.
    fn age_ratings(&self) -> HashMap<String, String> {
        let Ok(text) = self.read("age_ratings.tsv") else {
            return HashMap::new();
        };
        parse_age_ratings(&text)
            .into_iter()
            .filter_map(|record| record.age_rating.map(|cert| (record.imdb_id, cert)))
            .collect()
    }
}

impl ImdbFetch for FileImdbFetch {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        let mut records = parse_basics(&self.read("title.basics.tsv")?);
        let ages = self.age_ratings();
        for record in &mut records {
            record.age_rating = ages.get(&record.imdb_id).cloned();
        }
        Ok(records)
    }

    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        Ok(parse_ratings(&self.read("title.ratings.tsv")?))
    }

    fn by_id(&self, imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError> {
        if valid_id(imdb_id).is_none() {
            return Ok(None);
        }
        let merged = merge(self.basics()?, self.ratings()?);
        Ok(merged.into_iter().find(|record| record.imdb_id == imdb_id))
    }
}

/// Shared handle so a test can inspect the read count after moving the fetch
/// into a provider.
impl ImdbFetch for Arc<FileImdbFetch> {
    fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        (**self).basics()
    }
    fn ratings(&self) -> Result<Vec<ImdbRecord>, ImdbError> {
        (**self).ratings()
    }
    fn by_id(&self, imdb_id: &str) -> Result<Option<ImdbRecord>, ImdbError> {
        (**self).by_id(imdb_id)
    }
}

/// Parse the optional operator-supplied `age_ratings.tsv`. Header, invalid ids,
/// nulls and blank certifications are skipped; values are sanitised and capped.
pub fn parse_age_ratings(text: &str) -> Vec<ImdbRecord> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let Some(tconst) = cols.next() else { continue };
        if tconst == "tconst" {
            continue;
        }
        let Some(imdb_id) = valid_id(tconst) else {
            continue;
        };
        let Some(cert) = cols
            .next()
            .and_then(opt)
            .and_then(|raw| sanitize(raw, MAX_AGE_RATING_CHARS))
        else {
            continue;
        };
        out.push(ImdbRecord {
            imdb_id,
            age_rating: Some(cert),
            ..Default::default()
        });
    }
    out
}

/// Parse `title.basics.tsv`. Header, malformed rows and non-listing types are
/// skipped so one bad line cannot discard the whole dataset.
pub fn parse_basics(text: &str) -> Vec<ImdbRecord> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let Some(tconst) = cols.next() else { continue };
        if tconst == "tconst" {
            continue;
        }
        let Some(imdb_id) = valid_id(tconst) else {
            continue;
        };
        let title_type = cols.next().unwrap_or("");
        if !LISTING_TYPES.contains(&title_type) {
            continue;
        }
        let raw_title = cols.next().unwrap_or("");
        let _original_title = cols.next().unwrap_or("");
        let _is_adult = cols.next().unwrap_or("");
        let start_year = cols.next().unwrap_or("");
        let _end_year = cols.next().unwrap_or("");
        let _runtime = cols.next().unwrap_or("");
        let genres_raw = cols.next().unwrap_or("");

        let Some(title) = sanitize(raw_title, MAX_TITLE_CHARS) else {
            continue;
        };
        let year = opt(start_year)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|year| (1870..=2200).contains(year));
        let genres = parse_genres(genres_raw);

        out.push(ImdbRecord {
            imdb_id,
            title,
            year,
            genres,
            ..Default::default()
        });
    }
    out
}

/// Parse `title.ratings.tsv`; only id, rating and vote count are used.
pub fn parse_ratings(text: &str) -> Vec<ImdbRecord> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let Some(tconst) = cols.next() else { continue };
        if tconst == "tconst" {
            continue;
        }
        let Some(imdb_id) = valid_id(tconst) else {
            continue;
        };
        let rating = cols
            .next()
            .and_then(opt)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| (0.0..=10.0).contains(value));
        let votes = cols
            .next()
            .and_then(opt)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0);

        out.push(ImdbRecord {
            imdb_id,
            rating,
            votes,
            ..Default::default()
        });
    }
    out
}

/// Attach ratings to their basics listing by id. Ratings with no matching
/// listing are dropped; listings with no rating keep `None`.
pub fn merge(basics: Vec<ImdbRecord>, ratings: Vec<ImdbRecord>) -> Vec<ImdbRecord> {
    let by_id: HashMap<&str, &ImdbRecord> = ratings
        .iter()
        .map(|record| (record.imdb_id.as_str(), record))
        .collect();
    basics
        .into_iter()
        .map(|mut listing| {
            if let Some(rated) = by_id.get(listing.imdb_id.as_str()) {
                listing.rating = rated.rating;
                listing.votes = rated.votes;
            }
            listing
        })
        .collect()
}

/// IMDb's null token, plus blanks, become `None`.
fn opt(raw: &str) -> Option<&str> {
    let value = raw.trim();
    if value.is_empty() || value == "\\N" {
        None
    } else {
        Some(value)
    }
}

/// Only `tt` + digits is a usable id; it never becomes a path or URL.
fn valid_id(raw: &str) -> Option<String> {
    let value = raw.trim();
    let digits = value.strip_prefix("tt")?;
    if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(value.to_string())
}

fn parse_genres(raw: &str) -> Vec<String> {
    let mut genres = Vec::new();
    if opt(raw).is_none() {
        return genres;
    }
    for part in raw.split(',') {
        let Some(genre) = sanitize(part, MAX_GENRE_CHARS) else {
            continue;
        };
        if !genres
            .iter()
            .any(|known: &String| known.eq_ignore_ascii_case(&genre))
        {
            genres.push(genre);
            if genres.len() == MAX_GENRES {
                break;
            }
        }
    }
    genres
}

/// Collapse whitespace, drop control characters, cap length. Blank becomes
/// `None`. Same shape as the NFO/TMDB sanitisers already in this crate.
fn sanitize(raw: &str, max_chars: usize) -> Option<String> {
    let without_controls: String = raw
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .collect();
    let joined = without_controls
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return None;
    }
    let capped: String = joined.chars().take(max_chars).collect();
    if capped.trim().is_empty() {
        None
    } else {
        Some(capped)
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;

    const BASICS: &str = "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0111161\tmovie\tThe Shawshank Redemption\tThe Shawshank Redemption\t0\t1994\t\\N\t142\tDrama\n";
    const RATINGS: &str = "tconst\taverageRating\tnumVotes\n\
tt0111161\t9.3\t2500000\n";
    const AGES: &str = "tconst\tcertification\n\
tt0111161\tR\n";

    fn temp_dir(tag: &str) -> PathBuf {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("mh-imdb-file-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).expect("write fixture");
    }

    #[test]
    fn disabled_provider_never_reads_the_dataset_directory() {
        let dir = temp_dir("off");
        write(&dir, "title.basics.tsv", BASICS);
        write(&dir, "title.ratings.tsv", RATINGS);
        let fetch = Arc::new(FileImdbFetch::new(&dir));
        let provider = ImdbProvider::with_fetch(ImdbOptions::default(), Box::new(fetch.clone()));

        assert!(provider.records().is_none());
        assert!(provider.by_id("tt0111161").is_none());
        assert_eq!(
            fetch.read_count(),
            0,
            "opt-in off must not open any dataset file"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enabled_provider_reads_files_and_returns_display_fields() {
        let dir = temp_dir("on");
        write(&dir, "title.basics.tsv", BASICS);
        write(&dir, "title.ratings.tsv", RATINGS);
        write(&dir, "age_ratings.tsv", AGES);
        let fetch = Arc::new(FileImdbFetch::new(&dir));
        let provider =
            ImdbProvider::with_fetch(ImdbOptions { enabled: true }, Box::new(fetch.clone()));

        let record = provider.by_id("tt0111161").expect("merged record");
        assert_eq!(record.rating, Some(9.3));
        assert_eq!(record.votes, Some(2_500_000));
        assert_eq!(record.age_rating.as_deref(), Some("R"));
        assert!(
            fetch.read_count() > 0,
            "an enabled lookup must read the TSVs"
        );

        let records = provider.records().expect("records");
        let shawshank = records
            .iter()
            .find(|record| record.imdb_id == "tt0111161")
            .expect("listing");
        assert_eq!(shawshank.age_rating.as_deref(), Some("R"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_files_degrade_to_none() {
        let dir = temp_dir("missing");
        let provider = ImdbProvider::with_fetch(
            ImdbOptions { enabled: true },
            Box::new(FileImdbFetch::new(&dir)),
        );
        assert!(provider.records().is_none());
        assert!(provider.by_id("tt0111161").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn age_ratings_parser_skips_headers_nulls_and_hostile_values() {
        let rows = parse_age_ratings(
            "tconst\tcertification\n\
             tt0000001\tPG-13\n\
             tt0000002\t\\N\n\
             not-an-id\tR\n\
             tt0000003\tBad\u{0}Cert Here\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].age_rating.as_deref(), Some("PG-13"));
        assert_eq!(rows[1].age_rating.as_deref(), Some("BadCert Here"));
    }
}
