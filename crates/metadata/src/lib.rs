//! Offline metadata: Kodi-style `.nfo` sidecars read beside the media file.
//!
//! The NFO and offline IMDb paths need no network and never write beside the
//! media. Network-capable providers (TMDB, OMDb, keyless IMDb artwork) live in
//! their own modules behind explicit seams; poster URLs are allowlisted, bytes
//! are cached locally, and bundled art is the fallback.

pub mod cache;
pub mod imdb;
pub mod imdb_artwork;
pub mod model;
pub mod nfo;
pub mod omdb;
pub mod tmdb;

pub use cache::{RatingCache, CACHE_TTL, DEFAULT_IMDB_CACHE_MAX_BYTES};
pub use imdb::{
    merge, parse_age_ratings, parse_basics, parse_ratings, FileImdbFetch, ImdbFetch, ImdbOptions,
    ImdbProvider, MemoryFetch, IMDB_ATTRIBUTION,
};
// `imdb_search_fetch` returns an UNVALIDATED URL by contract: callers MUST run
// `validate_image_url` before using it (prefer `ImdbArtworkProvider::poster_url`).
pub use imdb_artwork::{
    fetch_image_bytes, imdb_search_fetch, validate_image_bytes, validate_image_url, ArtworkFetchFn,
    ImdbArtworkProvider,
};
pub use model::{ImdbError, ImdbRecord, NfoDocument, NfoError, MAX_NFO_BYTES};
pub use nfo::{parse, read_beside};
pub use omdb::{
    api_key_from_env, MemoryOmdbFetch, OmdbError, OmdbFetch, OmdbOptions, OmdbProvider, API_KEY_ENV,
};
pub use tmdb::{
    artwork_key, fetch_artwork_bytes, http_search_fetch, ArtworkCache, TmdbLookup, TmdbOptions,
    TmdbProvider, DEFAULT_MAX_CACHE_BYTES, MAX_ARTWORK_BYTES, TMDB_ATTRIBUTION,
};

use app_core::domain::{AppError, AppErrorCode, MetadataRecord};
use app_core::ports::MetadataProvider;
use async_trait::async_trait;
use std::path::Path;

/// Reads the sidecar that sits next to a media file. Stateless and cheap: a
/// missing file costs one failed metadata lookup, not an error.
#[derive(Debug, Default, Clone, Copy)]
pub struct NfoMetadataProvider;

impl NfoMetadataProvider {
    pub fn new() -> Self {
        Self
    }

    /// `Ok(None)` means there is no sidecar; `Err` means one was refused.
    pub fn lookup_path(&self, media: &Path) -> Result<Option<MetadataRecord>, NfoError> {
        read_beside(media).map(|document| document.map(NfoDocument::into_record))
    }
}

#[async_trait]
impl MetadataProvider for NfoMetadataProvider {
    fn name(&self) -> &'static str {
        "local-nfo"
    }

    async fn lookup(&self, path: &str) -> Result<Option<MetadataRecord>, AppError> {
        self.lookup_path(Path::new(path))
            .map_err(|err| AppError::new(AppErrorCode::Internal, err.to_string(), false))
    }
}
