//! `mediahaven-core`: the local HTTP backend for MEDIA_HAVEN.
//!
//! Layering: `app-core` owns the domain and ports, `media-persistence` owns
//! SQLite, and this crate owns transport only — routing, authentication and
//! process lifecycle. No policy lives here that the domain could own.

pub mod acquisition;
pub mod auth;
pub mod indexers;
pub mod routes;
pub mod seed;
pub mod server;
pub mod state;

pub use indexers::{BundledIndexer, BUNDLED_INDEXERS};
pub use server::{build_router, run, Config};
