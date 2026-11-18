//! `media-library`: folder import, safe scanning and metadata association.
//!
//! This crate owns filesystem traversal policy. It never performs whole-drive
//! scans and never modifies source files.

pub mod metadata;
pub mod path_policy;
pub mod scanner;

pub use metadata::FileMetadata;
pub use path_policy::{
    canonicalize_root, extension_of, is_media_extension, is_safe_entry, sanitize_title, within,
    PathError,
};
pub use scanner::{classify_kind, stable_id, ScanError, ScanOutcome, ScannedFile, Scanner};
