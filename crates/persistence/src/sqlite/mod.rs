//! SQLite adapter for the library ports.
//!
//! The adapter owns schema shape, transactions and SQL only. It performs no
//! policy decisions and no HTTP work, per ADR-0003 and the crate boundary.

pub mod library_repo;
pub mod pool;

pub use library_repo::SqliteLibraryRepository;
pub use pool::{connect, connect_path, run_migrations};
