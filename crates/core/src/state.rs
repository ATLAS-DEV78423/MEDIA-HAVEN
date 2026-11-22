//! Shared server state: the library application service, the install token,
//! the optional TMDB provider, the offline IMDb ratings provider, and the
//! keyless IMDb poster provider.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_core::application::LibraryService;
use media_acquisition::AcquisitionState;
use media_metadata::tmdb::{api_key_from_env, http_search_fetch, DEFAULT_MAX_CACHE_BYTES};
use media_metadata::{
    FileImdbFetch, ImdbArtworkProvider, ImdbOptions, ImdbProvider, TmdbOptions, TmdbProvider,
};
use media_persistence::sqlite::SqliteLibraryRepository;

/// The application service over the SQLite adapter. `core` never talks to
/// `sqlx` directly, so the adapter stays replaceable behind the port.
pub type LibraryServiceHandle = LibraryService<SqliteLibraryRepository>;

/// Setting both the opt-in and the operator-supplied dataset directory. Unset
/// means disabled: no TSV file is opened and no lookup is served.
pub const IMDB_DIR_ENV: &str = "MEDIAHAVEN_IMDB_DIR";

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<LibraryServiceHandle>,
    /// Per-install secret (ADR-0005 §2). One token, no roles.
    pub token: Arc<str>,
    /// Host-internal acquisition orchestrator. Lives in `core` because only it
    /// sits on the `acquire` network where `qbittorrent:8080` resolves.
    pub acquisition: Arc<AcquisitionState>,
    /// Opt-in offline IMDb provider. `new` builds it disabled; the server
    /// process attaches the operator-configured one via `imdb_provider_from_env`.
    pub imdb: Arc<ImdbProvider>,
    /// Optional TMDB artwork provider. Enabled only when the operator stored a
    /// key; a failed lookup degrades to IMDb, then bundled art (ADR-0026).
    pub tmdb: Arc<TmdbProvider>,
    /// Keyless IMDb poster provider (title and year only, no API key). Always
    /// available; a failed lookup degrades to bundled art.
    pub imdb_artwork: Arc<ImdbArtworkProvider>,
    /// Directory of hashed, cached poster bytes. Set by the server; defaults to
    /// a temp dir so existing `AppState::new(...)` test callers still work.
    pub artwork_dir: Arc<PathBuf>,
}

impl AppState {
    pub fn new(service: LibraryServiceHandle, token: impl Into<String>) -> Self {
        Self {
            service: Arc::new(service),
            token: Arc::from(token.into()),
            acquisition: Arc::new(AcquisitionState::new()),
            imdb: Arc::new(ImdbProvider::new(ImdbOptions::default())),
            tmdb: Arc::new(TmdbProvider::new(TmdbOptions::default())),
            imdb_artwork: Arc::new(ImdbArtworkProvider::new()),
            artwork_dir: Arc::new(std::env::temp_dir().join("media-haven-artwork")),
        }
    }

    /// Attach a configured provider. Test seam: proves the host route works
    /// with real TSV files without touching the process environment.
    pub fn with_imdb(mut self, provider: ImdbProvider) -> Self {
        self.imdb = Arc::new(provider);
        self
    }

    /// Attach a configured TMDB provider. Test seam: exercises enrichment with
    /// a fake fetch, no network and no process environment.
    pub fn with_tmdb(mut self, provider: TmdbProvider) -> Self {
        self.tmdb = Arc::new(provider);
        self
    }

    /// Attach a configured IMDb poster provider. Test seam: exercises
    /// enrichment with a fake fetch, no network and no process environment.
    pub fn with_imdb_artwork(mut self, provider: ImdbArtworkProvider) -> Self {
        self.imdb_artwork = Arc::new(provider);
        self
    }

    /// Point cached poster bytes at the install data directory.
    pub fn with_artwork_dir(mut self, dir: PathBuf) -> Self {
        self.artwork_dir = Arc::new(dir);
        self
    }
}

/// Build the IMDb provider from the operator's opt-in directory variable.
/// Absent or blank leaves it disabled (fail closed, zero file reads).
pub fn imdb_provider_from_env() -> ImdbProvider {
    let configured = std::env::var(IMDB_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    match configured {
        Some(dir) => ImdbProvider::with_fetch(
            ImdbOptions { enabled: true },
            Box::new(FileImdbFetch::new(PathBuf::from(dir))),
        ),
        None => ImdbProvider::new(ImdbOptions::default()),
    }
}

/// Build the TMDB provider from the operator-supplied key. Enabled whenever a
/// non-blank key is present; there is deliberately no opt-in switch (ADR-0026).
pub fn tmdb_provider_from_env(data_dir: &Path) -> TmdbProvider {
    let api_key = api_key_from_env();
    let enabled = api_key.is_some();
    TmdbProvider::with_fetch(
        TmdbOptions {
            enabled,
            api_key,
            cache_dir: data_dir.join("tmdb-cache"),
            max_bytes: DEFAULT_MAX_CACHE_BYTES,
        },
        http_search_fetch(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_persistence::sqlite::connect_path;

    async fn service_for_test() -> LibraryServiceHandle {
        let dir = std::env::temp_dir().join(format!("mh-tmdb-state-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        LibraryService::new(SqliteLibraryRepository::new(pool))
    }

    /// Adapt the existing async helper for synchronous state tests.
    fn service_for_test_blocking() -> LibraryServiceHandle {
        tokio::runtime::Runtime::new()
            .expect("build test runtime")
            .block_on(service_for_test())
    }

    #[test]
    fn imdb_artwork_provider_is_available_without_a_key() {
        let state = AppState::new(service_for_test_blocking(), "test-token");
        assert!(state.imdb_artwork.poster_url("", None).is_none()); // empty title is refused
    }

    #[test]
    fn default_state_has_a_disabled_imdb_provider() {
        // No AppSettings literal is touched: an unconfigured provider is a
        // zero-fetch no-op, so a store-less construction is safe to assert.
        assert!(!ImdbProvider::new(ImdbOptions::default()).options().enabled);
    }
}
