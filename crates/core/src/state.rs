//! Shared server state: the library application service and the install token.

use std::sync::Arc;

use app_core::application::LibraryService;
use media_acquisition::AcquisitionState;
use media_persistence::sqlite::SqliteLibraryRepository;

/// The application service over the SQLite adapter. `core` never talks to
/// `sqlx` directly, so the adapter stays replaceable behind the port.
pub type LibraryServiceHandle = LibraryService<SqliteLibraryRepository>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<LibraryServiceHandle>,
    /// Per-install secret (ADR-0005 §2). One token, no roles.
    pub token: Arc<str>,
    /// Host-internal acquisition orchestrator. Lives in `core` because only it
    /// sits on the `acquire` network where `qbittorrent:8080` resolves.
    pub acquisition: Arc<AcquisitionState>,
}

impl AppState {
    pub fn new(service: LibraryServiceHandle, token: impl Into<String>) -> Self {
        Self {
            service: Arc::new(service),
            token: Arc::from(token.into()),
            acquisition: Arc::new(AcquisitionState::new()),
        }
    }
}
