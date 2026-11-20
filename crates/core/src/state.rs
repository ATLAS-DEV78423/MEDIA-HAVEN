//! Shared server state: the library application service, the install token,
//! and the opt-in, offline IMDb provider.

use std::path::PathBuf;
use std::sync::Arc;

use app_core::application::LibraryService;
use media_acquisition::AcquisitionState;
use media_metadata::{FileImdbFetch, ImdbOptions, ImdbProvider};
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
}

impl AppState {
    pub fn new(service: LibraryServiceHandle, token: impl Into<String>) -> Self {
        Self {
            service: Arc::new(service),
            token: Arc::from(token.into()),
            acquisition: Arc::new(AcquisitionState::new()),
            imdb: Arc::new(ImdbProvider::new(ImdbOptions::default())),
        }
    }

    /// Attach a configured provider. Test seam: proves the host route works
    /// with real TSV files without touching the process environment.
    pub fn with_imdb(mut self, provider: ImdbProvider) -> Self {
        self.imdb = Arc::new(provider);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_has_a_disabled_imdb_provider() {
        // No AppSettings literal is touched: an unconfigured provider is a
        // zero-fetch no-op, so a store-less construction is safe to assert.
        assert!(!ImdbProvider::new(ImdbOptions::default()).options().enabled);
    }
}
