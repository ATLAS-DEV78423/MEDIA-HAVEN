//! Single source of truth for the renderer command surface (ADR-0005 §3).
//!
//! There is exactly one named command per backend renderer operation and no
//! generic passthrough. The list in [`COMMANDS`] is what the parity test checks
//! against the route table; the host signs with [`sign`] using the exact
//! canonical string the API verifies.
//!
//! This crate is deliberately dependency-light: it uses only crates already in
//! the workspace dependency table.

use std::fmt;
use std::str::FromStr;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Every renderer command, exactly once. `/healthz` and `/readyz` are probes,
/// not renderer commands, and are intentionally absent.
pub const COMMANDS: &[&str] = &[
    "library_home",
    "library_movies",
    "library_shows",
    "library_detail",
    "library_episodes",
    "library_search",
    "library_roots",
    "library_root_add",
    "library_root_remove",
    "library_scan",
    "library_watchlist",
    "watchlist_add",
    "watchlist_remove",
    "watchlist_reorder",
    "queue_list",
    "queue_add",
    "queue_remove",
    "queue_reorder",
    "downloads_list",
    "settings_get",
    "settings_set",
    "playback_start",
    "playback_stop",
    "playback_checkpoint",
];

/// Typed form of [`COMMANDS`]. No `strum` dependency: the mapping is written
/// once as a match and round-trips with [`COMMANDS`] in the parity test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    LibraryHome,
    LibraryMovies,
    LibraryShows,
    LibraryDetail,
    LibraryEpisodes,
    LibrarySearch,
    LibraryRoots,
    LibraryRootAdd,
    LibraryRootRemove,
    LibraryScan,
    LibraryWatchlist,
    WatchlistAdd,
    WatchlistRemove,
    WatchlistReorder,
    QueueList,
    QueueAdd,
    QueueRemove,
    QueueReorder,
    DownloadsList,
    SettingsGet,
    SettingsSet,
    PlaybackStart,
    PlaybackStop,
    PlaybackCheckpoint,
}

impl Command {
    /// The wire name, which is also the POST path minus the leading slash.
    pub fn as_str(self) -> &'static str {
        match self {
            Command::LibraryHome => "library_home",
            Command::LibraryMovies => "library_movies",
            Command::LibraryShows => "library_shows",
            Command::LibraryDetail => "library_detail",
            Command::LibraryEpisodes => "library_episodes",
            Command::LibrarySearch => "library_search",
            Command::LibraryRoots => "library_roots",
            Command::LibraryRootAdd => "library_root_add",
            Command::LibraryRootRemove => "library_root_remove",
            Command::LibraryScan => "library_scan",
            Command::LibraryWatchlist => "library_watchlist",
            Command::WatchlistAdd => "watchlist_add",
            Command::WatchlistRemove => "watchlist_remove",
            Command::WatchlistReorder => "watchlist_reorder",
            Command::QueueList => "queue_list",
            Command::QueueAdd => "queue_add",
            Command::QueueRemove => "queue_remove",
            Command::QueueReorder => "queue_reorder",
            Command::DownloadsList => "downloads_list",
            Command::SettingsGet => "settings_get",
            Command::SettingsSet => "settings_set",
            Command::PlaybackStart => "playback_start",
            Command::PlaybackStop => "playback_stop",
            Command::PlaybackCheckpoint => "playback_checkpoint",
        }
    }
}

/// A name that is not one of the declared renderer commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownCommand;

impl fmt::Display for UnknownCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown renderer command")
    }
}

impl std::error::Error for UnknownCommand {}

impl FromStr for Command {
    type Err = UnknownCommand;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "library_home" => Ok(Command::LibraryHome),
            "library_movies" => Ok(Command::LibraryMovies),
            "library_shows" => Ok(Command::LibraryShows),
            "library_detail" => Ok(Command::LibraryDetail),
            "library_episodes" => Ok(Command::LibraryEpisodes),
            "library_search" => Ok(Command::LibrarySearch),
            "library_roots" => Ok(Command::LibraryRoots),
            "library_root_add" => Ok(Command::LibraryRootAdd),
            "library_root_remove" => Ok(Command::LibraryRootRemove),
            "library_scan" => Ok(Command::LibraryScan),
            "library_watchlist" => Ok(Command::LibraryWatchlist),
            "watchlist_add" => Ok(Command::WatchlistAdd),
            "watchlist_remove" => Ok(Command::WatchlistRemove),
            "watchlist_reorder" => Ok(Command::WatchlistReorder),
            "queue_list" => Ok(Command::QueueList),
            "queue_add" => Ok(Command::QueueAdd),
            "queue_remove" => Ok(Command::QueueRemove),
            "queue_reorder" => Ok(Command::QueueReorder),
            "downloads_list" => Ok(Command::DownloadsList),
            "settings_get" => Ok(Command::SettingsGet),
            "settings_set" => Ok(Command::SettingsSet),
            "playback_start" => Ok(Command::PlaybackStart),
            "playback_stop" => Ok(Command::PlaybackStop),
            "playback_checkpoint" => Ok(Command::PlaybackCheckpoint),
            _ => Err(UnknownCommand),
        }
    }
}

/// Lowercase hex SHA-256 of the raw request body.
pub fn body_digest(body: &[u8]) -> String {
    hex::encode(Sha256::digest(body))
}

/// The exact string that is signed: `<timestamp>\n<METHOD>\n<path>\n<hex sha256(body)>`.
pub fn canonical_message(timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    format!("{timestamp}\n{method}\n{path}\n{}", body_digest(body))
}

/// Lowercase hex HMAC-SHA256 of [`canonical_message`], keyed with the bearer
/// token bytes (the same bytes the API uses as the HMAC key).
pub fn sign(token: &[u8], timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    // HMAC accepts keys of any length, so this cannot fail.
    let mut mac = HmacSha256::new_from_slice(token).expect("HMAC-SHA256 accepts any key length");
    mac.update(canonical_message(timestamp, method, path, body).as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// One duplicated-path finding from a library scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDuplicate {
    pub hash: String,
    pub paths: Vec<String>,
}

/// Response of `library_scan`. The backend builds this inline with `json!`;
/// the shape is pinned here so the host and renderer agree on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOutcome {
    pub scanned: i64,
    pub cancelled: bool,
    pub errors: Vec<String>,
    pub duplicates: Vec<ScanDuplicate>,
}
