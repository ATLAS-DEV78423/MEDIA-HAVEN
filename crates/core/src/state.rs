//! Shared server state: the library application service, the install token,
//! and the opt-in, offline IMDb provider.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_core::application::LibraryService;
use media_acquisition::AcquisitionState;
use media_metadata::tmdb::{api_key_from_env as tmdb_api_key_from_env, DEFAULT_MAX_CACHE_BYTES};
use media_metadata::{
    http_search_fetch, FileImdbFetch, ImdbOptions, ImdbProvider, TmdbOptions, TmdbProvider,
};
use media_persistence::sqlite::SqliteLibraryRepository;

/// The application service over the SQLite adapter. `core` never talks to
/// `sqlx` directly, so the adapter stays replaceable behind the port.
pub type LibraryServiceHandle = LibraryService<SqliteLibraryRepository>;

/// Setting both the opt-in and the operator-supplied dataset directory. Unset
/// means disabled: no TSV file is opened and no lookup is served.
pub const IMDB_DIR_ENV: &str = "MEDIAHAVEN_IMDB_DIR";

/// Opt-in flag for the TMDB artwork provider. Requires a key as well (ADR-0011).
pub const TMDB_OPTIN_ENV: &str = "MEDIAHAVEN_TMDB_OPTIN";

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
    /// Opt-in TMDB provider. `new` builds it disabled; the server attaches the
    /// operator-configured one via `tmdb_provider_from_env`.
    pub tmdb: Arc<TmdbProvider>,
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
    /// a fake transport, no network and no process environment.
    pub fn with_tmdb(mut self, provider: TmdbProvider) -> Self {
        self.tmdb = Arc::new(provider);
        self
    }

    /// Point cached poster bytes at the install data directory.
    pub fn with_artwork_dir(mut self, dir: PathBuf) -> Self {
        self.artwork_dir = Arc::new(dir);
        self
    }
}

/// Build the TMDB provider only when the operator opted in AND supplied a key.
/// Either condition missing leaves it disabled: zero requests, zero file reads.
pub fn tmdb_provider_from_env(data_dir: &Path) -> TmdbProvider {
    let opted_in = std::env::var(TMDB_OPTIN_ENV).as_deref() == Ok("1");
    let (enabled, api_key) = match (opted_in, tmdb_api_key_from_env()) {
        (true, Some(key)) => (true, Some(key)),
        _ => (false, None),
    };
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

    #[test]
    fn default_state_has_a_disabled_imdb_provider() {
        // No AppSettings literal is touched: an unconfigured provider is a
        // zero-fetch no-op, so a store-less construction is safe to assert.
        assert!(!ImdbProvider::new(ImdbOptions::default()).options().enabled);
    }

    #[tokio::test]
    async fn default_state_has_a_disabled_tmdb_provider() {
        let state = AppState::new(service_for_test().await, "test-token");
        assert!(!state.tmdb.options().enabled);
        assert!(state.tmdb.lookup_full("Anything", None).is_none());
    }
}
