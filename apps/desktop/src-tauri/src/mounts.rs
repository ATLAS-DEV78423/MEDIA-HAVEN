//! Media-root compose override rendering (Task A9).
//!
//! Each library root becomes exactly one read-only bind mount on `core`,
//! named `/media/root-<n>`. This module is **pure data**: it validates roots,
//! renders the override YAML fragment and the persisted container→host mount
//! map, and writes the fragment atomically. It never invokes a process — the
//! supervisor owns `compose up` and calls that only after the override is on
//! disk (ADR-0005 §3, docs/architecture/docker-workspace.md §4).
//!
//! The validation here is the lexical/containment layer. The read-only mount
//! is the backstop, not the first line of defence.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A library root as stored by the application: its id and host path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSpec {
    pub id: String,
    pub path: PathBuf,
}

impl RootSpec {
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
        }
    }
}

/// One rendered read-only mount and the host path it resolves to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedMount {
    pub container_path: String,
    pub host_path: PathBuf,
}

/// The rendered override: a YAML fragment plus the host-path map (data only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOutput {
    /// Compose override YAML fragment. No process call is made here.
    pub fragment: String,
    /// Container→host map, persisted for playback path resolution (B2).
    pub mounts: Vec<RenderedMount>,
}

impl RenderOutput {
    /// The persisted host-path map keyed by container path, for playback (B2).
    /// The renderer never sees this — it stays inside the host process.
    pub fn host_path_map(&self) -> BTreeMap<String, PathBuf> {
        self.mounts
            .iter()
            .map(|m| (m.container_path.clone(), m.host_path.clone()))
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("root '{id}' path '{path}' is not a directory")]
    NotDirectory { id: String, path: String },
    #[error("root '{id}' is a reparse point (symlink or junction) and is refused")]
    ReparsePoint { id: String },
    #[error("root '{id}' is a filesystem or drive root and would widen the mount")]
    FilesystemRoot { id: String },
    #[error("root '{id}' is nested inside root '{other}'")]
    NestedRoot { id: String, other: String },
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

/// `FILE_ATTRIBUTE_REPARSE_POINT`: set on junctions and symlinks alike.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// True when `path` is a Windows reparse point (junction, symlink, mount point).
///
/// Reads the raw attributes from `symlink_metadata` so the link itself — not
/// its target — is inspected. This catches junctions that `is_symlink()`
/// alone can miss. Using `std`'s `MetadataExt` avoids adding any crate feature.
#[cfg(windows)]
fn is_reparse_point(path: &Path) -> io::Result<bool> {
    use std::os::windows::fs::MetadataExt;
    let attrs = std::fs::symlink_metadata(path)?.file_attributes();
    Ok(attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0)
}

#[cfg(not(windows))]
fn is_reparse_point(path: &Path) -> io::Result<bool> {
    Ok(std::fs::symlink_metadata(path)?.file_type().is_symlink())
}

/// Validate one root and return its canonical host path.
fn validate_root(spec: &RootSpec) -> Result<PathBuf, MountError> {
    // R-24: a drive root (`C:\`) or `/` has no parent — refuse before any
    // filesystem call so it can never become a widening mount.
    if spec.path.parent().is_none() {
        return Err(MountError::FilesystemRoot {
            id: spec.id.clone(),
        });
    }

    // Refuse reparse points before canonicalising, which would follow them.
    if is_reparse_point(&spec.path)? {
        return Err(MountError::ReparsePoint {
            id: spec.id.clone(),
        });
    }

    // A root must be an existing directory, not a file.
    if !std::fs::metadata(&spec.path)?.is_dir() {
        return Err(MountError::NotDirectory {
            id: spec.id.clone(),
            path: spec.path.to_string_lossy().into_owned(),
        });
    }

    // Canonicalise for containment checks and for the persisted host path.
    let canonical = std::fs::canonicalize(&spec.path)?;
    if canonical.parent().is_none() {
        return Err(MountError::FilesystemRoot {
            id: spec.id.clone(),
        });
    }
    Ok(canonical)
}

/// Validate `roots` and render the override fragment plus the host-path map.
///
/// Container paths are assigned by position: `/media/root-0`, `/media/root-1`,
/// … One root yields exactly one read-only mount. Removing a root and calling
/// this again yields a fragment without it, so the next `up` drops that mount.
pub fn render_mounts(roots: &[RootSpec]) -> Result<RenderOutput, MountError> {
    let mut canonical: Vec<(String, PathBuf)> = Vec::with_capacity(roots.len());
    for spec in roots {
        canonical.push((spec.id.clone(), validate_root(spec)?));
    }

    // Refuse any root nested inside another (or an exact duplicate).
    for (i, (id, path)) in canonical.iter().enumerate() {
        for (j, (other_id, other)) in canonical.iter().enumerate() {
            if i != j && path.starts_with(other) {
                return Err(MountError::NestedRoot {
                    id: id.clone(),
                    other: other_id.clone(),
                });
            }
        }
    }

    let mut mounts = Vec::with_capacity(canonical.len());
    let mut lines = Vec::with_capacity(canonical.len());
    for (n, (_, host)) in canonical.iter().enumerate() {
        let container_path = format!("/media/root-{n}");
        let spec = format!("{}:{container_path}:ro", host.to_string_lossy());
        // Quote the whole spec as one JSON string, which is also a valid YAML
        // double-quoted scalar. This escapes backslashes, quotes and colons,
        // so no path can break out of the YAML fragment.
        let quoted = serde_json::to_string(&spec).expect("a String is always serialisable");
        lines.push(format!("      - {quoted}"));
        mounts.push(RenderedMount {
            container_path,
            host_path: host.clone(),
        });
    }

    let volumes = if lines.is_empty() {
        "    volumes: []".to_string()
    } else {
        format!("    volumes:\n{}", lines.join("\n"))
    };
    let fragment = format!("services:\n  core:\n{volumes}\n");

    Ok(RenderOutput { fragment, mounts })
}

/// Write `content` to `path` atomically: a sibling temp file is written in
/// full, then renamed over the destination. The previous override is left
/// intact until the rename, so an interrupted write cannot leave a partial
/// (and possibly wider) mount set on disk.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), MountError> {
    let file_name = path.file_name().unwrap_or_else(|| OsStr::new("override"));
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// A unique, existing directory under the system temp dir for a test.
    fn temp_dir(label: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mediahaven-a9-{label}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn mount_scope_one_mount_per_root_and_nothing_else() {
        let base = temp_dir("scope");
        let movies = base.join("movies");
        let shows = base.join("shows");
        std::fs::create_dir_all(&movies).unwrap();
        std::fs::create_dir_all(&shows).unwrap();

        let one = render_mounts(&[RootSpec::new("r1", &movies)]).expect("one root renders");
        assert_eq!(one.mounts.len(), 1, "one root must add exactly one mount");
        assert_eq!(one.mounts[0].container_path, "/media/root-0");
        assert_eq!(
            one.mounts[0].host_path,
            std::fs::canonicalize(&movies).unwrap()
        );
        assert_eq!(one.fragment.matches(":ro").count(), 1);

        let two = render_mounts(&[RootSpec::new("r1", &movies), RootSpec::new("r2", &shows)])
            .expect("two roots render");
        assert_eq!(two.mounts.len(), 2, "two roots must add exactly two mounts");
        assert_eq!(two.mounts[0].container_path, "/media/root-0");
        assert_eq!(two.mounts[1].container_path, "/media/root-1");
        assert_eq!(two.fragment.matches(":ro").count(), 2);
        assert!(two.fragment.starts_with("services:\n  core:\n"));
        assert!(
            two.fragment.contains("\"/media/root-0:ro\"")
                || two.fragment.contains("/media/root-0:ro")
        );

        // The persisted map has one entry per mount, keyed by container path.
        let map = two.host_path_map();
        assert_eq!(map.len(), 2);
        assert_eq!(map["/media/root-1"], std::fs::canonicalize(&shows).unwrap());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nested_root_is_refused() {
        let base = temp_dir("nested");
        let parent = base.join("movies");
        let child = parent.join("sub");
        std::fs::create_dir_all(&child).unwrap();

        let result = render_mounts(&[
            RootSpec::new("parent", &parent),
            RootSpec::new("child", &child),
        ]);
        assert!(
            matches!(result, Err(MountError::NestedRoot { .. })),
            "a root inside another root must be refused, got {result:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn renamed_root_disappears_from_next_render() {
        let base = temp_dir("rename");
        let movies = base.join("movies");
        let old = base.join("shows");
        let new = base.join("renamed");
        for dir in [&movies, &old, &new] {
            std::fs::create_dir_all(dir).unwrap();
        }

        let before = render_mounts(&[RootSpec::new("r1", &movies), RootSpec::new("r2", &old)])
            .expect("before render");
        assert_eq!(before.mounts.len(), 2);

        // "shows" is removed and replaced by "renamed" in the stored roots.
        let after = render_mounts(&[RootSpec::new("r1", &movies), RootSpec::new("r3", &new)])
            .expect("after render");
        let old_canon = std::fs::canonicalize(&old)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            after
                .mounts
                .iter()
                .all(|m| m.host_path.to_string_lossy() != old_canon),
            "the removed root must not survive in the next fragment/map"
        );
        assert_eq!(after.mounts.len(), 2);
        assert_eq!(
            after.mounts[1].host_path,
            std::fs::canonicalize(&new).unwrap()
        );
        assert!(!after.fragment.contains(&old_canon));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn filesystem_root_is_refused_r24() {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        let result = render_mounts(&[RootSpec::new("wide", root)]);
        assert!(
            matches!(result, Err(MountError::FilesystemRoot { .. })),
            "a drive/filesystem root must be refused (R-24), got {result:?}"
        );
    }

    #[test]
    fn non_directory_and_missing_paths_are_refused() {
        let base = temp_dir("nondir");
        let file = base.join("file.txt");
        std::fs::write(&file, b"not a directory").unwrap();

        let as_file = render_mounts(&[RootSpec::new("f", &file)]);
        assert!(matches!(as_file, Err(MountError::NotDirectory { .. })));

        let missing = render_mounts(&[RootSpec::new("m", base.join("does-not-exist"))]);
        assert!(matches!(missing, Err(MountError::Io(_))));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ordinary_directory_is_not_a_reparse_point() {
        let base = temp_dir("reparse");
        assert!(
            !is_reparse_point(&base).expect("stat a normal directory"),
            "a freshly created directory must not be flagged as a reparse point"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_atomic_replaces_an_existing_override() {
        let base = temp_dir("atomic");
        let target = base.join("override.yaml");
        std::fs::write(&target, b"old").unwrap();

        write_atomic(&target, "new contents").expect("atomic write");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new contents");
        assert!(!target.with_file_name("override.yaml.tmp").exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
