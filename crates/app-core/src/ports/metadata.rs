//! The metadata port. Providers are interchangeable; policy that decides
//! *whether* a provider may run lives in the application layer, not here.

use crate::domain::{AppError, MetadataRecord};
use async_trait::async_trait;

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    /// Stable name for diagnostics; also what an opt-in screen would show.
    fn name(&self) -> &'static str;

    /// Look up metadata for one source path.
    ///
    /// `Ok(None)` means the provider has nothing to say about this file. `Err`
    /// means the source was present but refused (oversized, malformed,
    /// hostile), which the caller reports rather than swallowing.
    async fn lookup(&self, path: &str) -> Result<Option<MetadataRecord>, AppError>;
}
