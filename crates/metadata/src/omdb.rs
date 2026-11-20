//! Optional OMDb age-rating provider — OFF by default, fetch seam only.
//!
//! No HTTP/TLS crate exists in the workspace and adding one needs an ADR, so
//! this crate never performs a request. The default fetch fails closed with
//! `Unavailable`; the app injects a transport only after the operator opts in.
//! Only an IMDb id and the injected key reach that fetch — never a file path,
//! folder name or library content. The key comes from the env/secret store and
//! is never read from or written to the repository.

use crate::cache::{RatingCache, DEFAULT_IMDB_CACHE_MAX_BYTES};
use crate::model::MAX_AGE_RATING_CHARS;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Environment variable the app populates from the ADR-0007 secret store.
pub const API_KEY_ENV: &str = "MEDIAHAVEN_OMDB_API_KEY";

/// Failure to obtain a rating is a refusal, never an invented certificate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OmdbError {
    /// Opt-in off, key missing, or no transport injected. No call was made.
    #[error("OMDb age ratings are unavailable")]
    Unavailable,
    #[error("OMDb fetch failed")]
    Fetch(String),
    #[error("OMDb returned an invalid age rating")]
    Invalid,
}

/// Read the key from the environment. Absent/blank means fail closed.
pub fn api_key_from_env() -> Option<String> {
    std::env::var(API_KEY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone)]
pub struct OmdbOptions {
    /// Master switch. `false` makes every call a zero-fetch no-op.
    pub enabled: bool,
    /// Injected secret. Never a literal in source or a renderer DTO field.
    pub api_key: Option<String>,
    pub max_cache_bytes: u64,
}

impl Default for OmdbOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            max_cache_bytes: DEFAULT_IMDB_CACHE_MAX_BYTES,
        }
    }
}

/// Pluggable transport seam. Receives only a validated IMDb id and the key.
/// `Ok(None)` means OMDb has no certification for the id.
pub trait OmdbFetch {
    fn certification(&self, imdb_id: &str, api_key: &str) -> Result<Option<String>, OmdbError>;
}

/// Fail-closed default: no key, no transport, no egress.
#[derive(Debug, Default)]
struct UnconfiguredFetch;

impl OmdbFetch for UnconfiguredFetch {
    fn certification(&self, _imdb_id: &str, _api_key: &str) -> Result<Option<String>, OmdbError> {
        Err(OmdbError::Unavailable)
    }
}

pub struct OmdbProvider {
    options: OmdbOptions,
    fetch: Box<dyn OmdbFetch>,
    cache: Mutex<RatingCache>,
}

impl OmdbProvider {
    pub fn new(options: OmdbOptions) -> Self {
        Self::with_fetch(options, Box::new(UnconfiguredFetch))
    }

    pub fn with_fetch(options: OmdbOptions, fetch: Box<dyn OmdbFetch>) -> Self {
        let cache = RatingCache::new(options.max_cache_bytes);
        Self {
            options,
            fetch,
            cache: Mutex::new(cache),
        }
    }

    pub fn options(&self) -> &OmdbOptions {
        &self.options
    }

    /// Sanitised certification for an IMDb id, or `Ok(None)` when OMDb has no
    /// rating. `Err(Unavailable)` when disabled or without a key — no fetch is
    /// attempted in either case, so no request can leave the machine.
    pub fn age_rating_for(&self, imdb_id: &str) -> Result<Option<String>, OmdbError> {
        if !self.options.enabled {
            return Err(OmdbError::Unavailable);
        }
        let key = self
            .options
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(OmdbError::Unavailable)?;
        // Reuse the IMDb id shape so a hostile caller cannot send arbitrary
        // strings (or anything path-like) to the injected transport.
        if !valid_id(imdb_id) {
            return Ok(None);
        }

        if let Some(cached) = self.cache.lock().ok().and_then(|cache| cache.get(imdb_id)) {
            return Ok(Some(cached));
        }

        let raw = match self.fetch.certification(imdb_id, key) {
            Ok(value) => value,
            Err(OmdbError::Fetch(_)) => return Err(OmdbError::Unavailable),
            Err(err) => return Err(err),
        };
        let Some(certification) = raw.as_deref().and_then(sanitize) else {
            return Ok(None);
        };
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(imdb_id, &certification);
        }
        Ok(Some(certification.to_string()))
    }
}

/// In-memory `OmdbFetch` for tests and downstream seams. The call counter
/// proves that a disabled provider or missing key makes zero requests.
#[derive(Debug, Default)]
pub struct MemoryOmdbFetch {
    ratings: Vec<(String, String)>,
    calls: Arc<AtomicUsize>,
}

impl MemoryOmdbFetch {
    pub fn new(ratings: Vec<(String, String)>) -> Self {
        Self {
            ratings,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl OmdbFetch for MemoryOmdbFetch {
    fn certification(&self, imdb_id: &str, _api_key: &str) -> Result<Option<String>, OmdbError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .ratings
            .iter()
            .find(|(id, _)| id == imdb_id)
            .map(|(_, rating)| rating.clone()))
    }
}

/// Shared handle so a test can inspect calls after moving the fetch into a
/// provider.
impl OmdbFetch for Arc<MemoryOmdbFetch> {
    fn certification(&self, imdb_id: &str, api_key: &str) -> Result<Option<String>, OmdbError> {
        (**self).certification(imdb_id, api_key)
    }
}

/// Only `tt` + digits is a usable IMDb id. Same shape as the dataset parser.
fn valid_id(raw: &str) -> bool {
    let Some(digits) = raw.trim().strip_prefix("tt") else {
        return false;
    };
    !digits.is_empty() && digits.len() <= 10 && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// Collapse whitespace, drop control characters, cap length. Blank becomes
/// `None`. Mirrors the IMDb/NFO sanitisers in this crate.
fn sanitize(raw: &str) -> Option<String> {
    let without_controls: String = raw.chars().filter(|ch| !ch.is_control()).collect();
    let joined = without_controls
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return None;
    }
    let capped: String = joined.chars().take(MAX_AGE_RATING_CHARS).collect();
    if capped.trim().is_empty() {
        None
    } else {
        Some(capped)
    }
}
