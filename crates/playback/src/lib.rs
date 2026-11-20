//! Host-side playback adapter (ADR-0002, ADR-0015).
//!
//! Two concerns live here:
//!
//! * [`mpv::Player`] / [`mpv::MpvPlayer`] — the playback port and its mpv
//!   implementation, which spawns mpv with argument vectors only and kills the
//!   child when the player is dropped (app exit / window close).
//! * [`resolve_source`] — the *only* bridge from a container-path identity to a
//!   host file (ADR-0015). It resolves through A9's
//!   `RenderOutput::host_path_map()` and fails closed.
//!
//! **Boundary rule (ADR-0015):** no filesystem path and no source token appears
//! in any renderer-facing DTO. [`resolve_source`] returns a host `PathBuf` that
//! stays inside the host process and is handed only to [`mpv::Player::open`].
//! Nothing in this crate derives `Serialize` over a path, so no path can leak
//! into the WebView contract.

pub mod ipc;
pub mod mpv;

use std::collections::BTreeMap;
use std::path::PathBuf;

/// A container-path identity could not be resolved to a file under a declared
/// read-only media root (ADR-0015 failure mode).
///
/// Resolution fails closed: the host never guesses a path and never passes a
/// path back to the renderer. The caller surfaces this as a legible
/// `source unavailable` state (ADR-0005 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("source unavailable: the media id no longer maps to a file under a declared root")]
pub enum SourceError {
    Unavailable,
}

/// Resolve a container-path identity to a host file under a declared root.
///
/// `map` is A9's `RenderOutput::host_path_map()`: keys are container roots
/// (`/media/root-<n>`), values are the canonical host root directories. The
/// incoming `container_path` is the item's full container path (e.g.
/// `/media/root-0/movies/hello.mkv`). The longest containing root key is found
/// via ordered lookup, its remainder is translated onto the host root, and the
/// candidate is canonicalised and checked to be an existing file *under that
/// root*. Any miss, missing file, or escape fails closed with
/// [`SourceError::Unavailable`]; no path is guessed and nothing is returned to
/// the renderer.
pub fn resolve_source(
    container_path: &str,
    map: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf, SourceError> {
    // Longest containing root key. BTreeMap is ordered, so keys that contain
    // `container_path` are a prefix; keep the longest one (nested mounts).
    let mut chosen: Option<(&str, &PathBuf)> = None;
    for (key, root) in map.iter() {
        let is_root = container_path == key.as_str()
            || container_path
                .strip_prefix(key.as_str())
                .is_some_and(|rest| rest.starts_with('/'));
        if is_root && chosen.is_none_or(|(best, _)| key.len() > best.len()) {
            chosen = Some((key.as_str(), root));
        }
    }
    let (key, host_root) = chosen.ok_or(SourceError::Unavailable)?;

    // Translate the container remainder onto the host root. Forward slashes
    // from the container view become the host platform's separator.
    let remainder = container_path[key.len()..].trim_start_matches('/');
    let candidate = if remainder.is_empty() {
        host_root.clone()
    } else {
        host_root.join(remainder.replace('/', std::path::MAIN_SEPARATOR_STR))
    };

    // Fail closed on every filesystem surprise: the root must exist, the file
    // must exist and be a file, and the resolved file must stay under the
    // canonical root (this catches `..`, symlinks/junctions and case tricks).
    let root_canon = std::fs::canonicalize(host_root).map_err(|_| SourceError::Unavailable)?;
    let file_canon = std::fs::canonicalize(&candidate).map_err(|_| SourceError::Unavailable)?;
    if !file_canon.is_file() || !file_canon.starts_with(&root_canon) {
        return Err(SourceError::Unavailable);
    }
    Ok(file_canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mediahaven-b2-{label}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn resolves_a_file_under_a_declared_root() {
        let base = temp_dir("valid");
        let root = base.join("movies");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub").join("hello.mkv");
        std::fs::write(&file, b"fake media").unwrap();

        let map = BTreeMap::from([("/media/root-0".to_string(), root.clone())]);
        let resolved = resolve_source("/media/root-0/sub/hello.mkv", &map)
            .expect("a real file under the root resolves");

        assert_eq!(resolved, std::fs::canonicalize(&file).unwrap());
        assert!(resolved.is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stale_or_missing_container_path_fails_closed() {
        let base = temp_dir("stale");
        let root = base.join("movies");
        std::fs::create_dir_all(&root).unwrap();
        let map = BTreeMap::from([("/media/root-0".to_string(), root.clone())]);

        // No root key matches at all.
        assert_eq!(
            resolve_source("/media/root-9/hello.mkv", &map),
            Err(SourceError::Unavailable)
        );
        // A matching root, but the file was moved/deleted on the host.
        assert_eq!(
            resolve_source("/media/root-0/gone.mkv", &map),
            Err(SourceError::Unavailable)
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn path_escaping_the_mapped_root_fails_closed() {
        let base = temp_dir("escape");
        let root = base.join("movies");
        std::fs::create_dir_all(&root).unwrap();
        // A real file *outside* the root, so canonicalise succeeds and the
        // containment check — not a missing-file error — is what rejects it.
        let outside = base.join("secret.mkv");
        std::fs::write(&outside, b"outside").unwrap();

        let map = BTreeMap::from([("/media/root-0".to_string(), root.clone())]);
        assert_eq!(
            resolve_source("/media/root-0/../secret.mkv", &map),
            Err(SourceError::Unavailable),
            "a resolved path outside the root must be refused"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sibling_prefix_root_does_not_match() {
        let base = temp_dir("prefix");
        let root = base.join("movies");
        std::fs::create_dir_all(&root).unwrap();
        let map = BTreeMap::from([("/media/root-0".to_string(), root.clone())]);
        // `/media/root-01` is not under `/media/root-0`.
        assert_eq!(
            resolve_source("/media/root-01/hello.mkv", &map),
            Err(SourceError::Unavailable)
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
