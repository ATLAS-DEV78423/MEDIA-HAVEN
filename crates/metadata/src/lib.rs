//! Offline metadata: Kodi-style `.nfo` sidecars read beside the media file.
//!
//! This crate performs no network access and never writes beside the media. A
//! network provider (TMDB) is a separate implementor of the same port, added
//! only behind an explicit, off-by-default opt-in.

pub mod cache;
pub mod model;
pub mod nfo;
pub mod tmdb;

pub use model::{NfoDocument, NfoError, MAX_NFO_BYTES};
pub use nfo::{parse, read_beside};
pub use tmdb::{TmdbLookup, TmdbOptions, TmdbProvider, TMDB_ATTRIBUTION};

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
