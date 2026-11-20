// F3 cross-cutting path-traversal / root-containment suite.
//
// Each case asserts BOTH that the escape is rejected AND that nothing outside
// the root was read into the library, modified or deleted. This file is wired
// as the `security` integration target of `media-library` via a one-line
// `include!` shim (crates/media-library/tests/security.rs) because the
// workspace root has no package to own a `tests/` directory.

use media_library::{canonicalize_root, is_safe_entry, within, PathError, Scanner};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "mh-traversal-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&p).expect("create temp dir");
    p
}

fn scan(root: &Path) -> media_library::ScanOutcome {
    Scanner::new().scan(root, &Arc::new(AtomicBool::new(false)))
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// No outside file may be indexed or even reported as an access attempt.
fn assert_not_indexed(out: &media_library::ScanOutcome, outside_name: &str) {
    assert!(
        out.files
            .iter()
            .all(|f| !Path::new(&f.path).ends_with(outside_name)),
        "outside file entered the library: {outside_name}"
    );
    assert!(
        out.errors
            .iter()
            .all(|e| !e.path.contains(outside_name) && !e.message.contains(outside_name)),
        "scanner reported access to outside target: {outside_name}"
    );
}

/// No outside file may be changed or deleted by a scan.
fn assert_outside_untouched(path: &Path, original: &[u8]) {
    assert!(
        path.exists(),
        "outside file was removed: {}",
        path.display()
    );
    assert_eq!(
        fs::read(path).unwrap(),
        original,
        "outside file was modified: {}",
        path.display()
    );
}

fn cleanup(a: &Path, b: &Path) {
    let _ = fs::remove_dir_all(a);
    let _ = fs::remove_dir_all(b);
}

fn make_symlink(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (target, link);
        false
    }
}

fn make_dir_symlink(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (target, link);
        false
    }
}

#[test]
fn dotdot_traversal_is_rejected_without_outside_access() {
    let root = tmp("dotdot-root");
    let outside = tmp("dotdot-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"DOTDOT-SECRET";
    fs::write(&secret, bytes).unwrap();

    let escape = root
        .join("..")
        .join(outside.file_name().unwrap())
        .join("secret.mkv");
    assert!(!within(&root, &escape), ".. candidate resolved inside root");
    assert!(!is_safe_entry(&root, &escape), ".. entry was accepted");

    let out = scan(&root);
    assert_not_indexed(&out, "secret.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn absolute_path_outside_root_is_rejected_without_outside_access() {
    let root = tmp("abs-root");
    let outside = tmp("abs-out");
    let secret = outside.join("secret.mp4");
    let bytes = b"ABSOLUTE-SECRET";
    fs::write(&secret, bytes).unwrap();

    assert!(!within(&root, &secret), "absolute outside path accepted");
    assert!(
        !is_safe_entry(&root, &secret),
        "absolute outside entry accepted"
    );

    let out = scan(&root);
    assert_not_indexed(&out, "secret.mp4");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn unc_path_is_rejected_without_outside_access() {
    let root = tmp("unc-root");
    let outside = tmp("unc-out");
    let secret = outside.join("sentinel.mkv");
    let bytes = b"UNC-SENTINEL";
    fs::write(&secret, bytes).unwrap();

    // Device-namespace and server-share forms must never pass containment.
    for unc in [
        Path::new(r"\\?\C:\Windows\System32\secret.mkv"),
        Path::new(r"\\attacker\share\secret.mkv"),
    ] {
        assert!(!within(&root, unc), "UNC path accepted: {}", unc.display());
        assert!(
            !is_safe_entry(&root, unc),
            "UNC entry accepted: {}",
            unc.display()
        );
    }

    let out = scan(&root);
    assert!(out.files.is_empty(), "UNC root indexed local files");
    assert_not_indexed(&out, "sentinel.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn symlink_junction_escape_is_refused_without_reading_target() {
    let root = tmp("link-root");
    let outside = tmp("link-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"SYMLINK-SECRET";
    fs::write(&secret, bytes).unwrap();
    let link = root.join("bait.mkv");
    let link_dir = root.join("baitdir");

    if !make_symlink(&secret, &link) || !make_dir_symlink(&outside, &link_dir) {
        // No privilege on this host: cannot exercise the refusal path.
        eprintln!("NOT EXERCISED: symlink/junction creation not permitted here");
        assert!(!is_safe_entry(&root, &link), "missing link was accepted");
        let out = scan(&root);
        assert!(out.files.is_empty(), "root had unexpected indexed files");
        assert_not_indexed(&out, "secret.mkv");
        assert_outside_untouched(&secret, bytes);
        cleanup(&root, &outside);
        return;
    }

    assert!(!is_safe_entry(&root, &link), "symlink entry accepted");
    assert_eq!(
        canonicalize_root(&link_dir),
        Err(PathError::SymlinkOrReparse),
        "symlink root was accepted"
    );

    let out = scan(&root);
    let outside_hash = sha256(bytes);
    assert!(
        out.files
            .iter()
            .all(|f| f.hash.as_deref() != Some(outside_hash.as_str())),
        "outside bytes were read through the symlink"
    );
    assert!(
        out.files.iter().all(|f| !f.path.contains("bait")),
        "symlink/junction was indexed"
    );
    assert_not_indexed(&out, "secret.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[cfg(windows)]
#[test]
fn junction_escape_is_refused_without_reading_target() {
    use std::process::Command;

    let root = tmp("junction-root");
    let outside = tmp("junction-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"JUNCTION-SECRET";
    fs::write(&secret, bytes).unwrap();
    let link_dir = root.join("junction_bait");

    // Junctions need no SeCreateSymbolicLinkPrivilege, unlike symlink_dir.
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link_dir)
        .arg(&outside)
        .status();
    if !matches!(status, Ok(s) if s.success()) {
        eprintln!("NOT EXERCISED: junction creation unavailable on this host");
        let out = scan(&root);
        assert_not_indexed(&out, "secret.mkv");
        assert_outside_untouched(&secret, bytes);
        cleanup(&root, &outside);
        return;
    }

    assert!(!is_safe_entry(&root, &link_dir), "junction entry accepted");
    assert_eq!(
        canonicalize_root(&link_dir),
        Err(PathError::SymlinkOrReparse),
        "junction root was accepted"
    );

    let out = scan(&root);
    let outside_hash = sha256(bytes);
    assert!(
        out.files
            .iter()
            .all(|f| f.hash.as_deref() != Some(outside_hash.as_str())),
        "outside bytes were read through the junction"
    );
    assert_not_indexed(&out, "secret.mkv");

    // remove_dir drops the junction itself, never its target.
    let _ = fs::remove_dir(&link_dir);
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn case_tricks_do_not_bypass_containment() {
    let root = tmp("case-root");
    let outside = tmp("case-out");
    let secret = outside.join("SECRET.mkv");
    let bytes = b"CASE-SECRET";
    fs::write(&secret, bytes).unwrap();

    let escape = root
        .join("SUB")
        .join("..")
        .join("..")
        .join(outside.file_name().unwrap())
        .join("SECRET.mkv");
    assert!(
        !within(&root, &escape),
        "case-trick .. resolved inside root"
    );
    assert!(!is_safe_entry(&root, &escape), "case-trick entry accepted");

    fs::write(root.join("MOVIE.MP4"), b"x").unwrap();
    let out = scan(&root);
    assert_eq!(out.files.len(), 1, "inside media file was not scanned");
    assert_eq!(out.files[0].extension.as_deref(), Some("mp4"));
    let canon = root.canonicalize().unwrap();
    assert!(
        Path::new(&out.files[0].path).starts_with(&canon),
        "indexed path is not confined to root"
    );
    assert_not_indexed(&out, "SECRET.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn unicode_normalisation_does_not_create_an_escape() {
    let root = tmp("uni-root");
    let outside = tmp("uni-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"UNICODE-SECRET";
    fs::write(&secret, bytes).unwrap();

    // Fullwidth full stops look like `..` but are ordinary filename bytes.
    let lookalike = format!("{}.{}.mkv", '\u{FF0E}', '\u{FF0E}');
    fs::write(root.join(&lookalike), b"x").unwrap();
    // One-dot leaders are not path separators / parent references either.
    let leader = root.join("\u{2024}\u{2024}").join("secret.mkv");
    assert!(
        !within(&root, &leader),
        "Unicode lookalike resolved outside"
    );
    assert!(!is_safe_entry(&root, &leader), "Unicode entry accepted");

    let out = scan(&root);
    assert_eq!(out.files.len(), 1, "Unicode filename was not scanned");
    assert!(
        !out.files[0]
            .path
            .split(['/', '\\'])
            .any(|component| component == ".."),
        ".. component leaked from Unicode name"
    );
    assert_not_indexed(&out, "secret.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn long_paths_do_not_escape_or_panic() {
    let root = tmp("long-root");
    let outside = tmp("long-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"LONG-SECRET";
    fs::write(&secret, bytes).unwrap();

    let long_segment = "a".repeat(300);
    let _ = fs::write(root.join(format!("{long_segment}.mkv")), b"x");
    let nested = root
        .join(&long_segment)
        .join("..")
        .join("..")
        .join(outside.file_name().unwrap())
        .join("secret.mkv");
    assert!(!within(&root, &nested), "long path resolved outside root");
    assert!(!is_safe_entry(&root, &nested), "long path entry accepted");

    let out = scan(&root);
    assert!(
        out.files.iter().all(|f| !f.path.contains("..")),
        ".. leaked through a long path"
    );
    assert_not_indexed(&out, "secret.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn wildcard_injection_is_literal_not_a_glob_escape() {
    let root = tmp("glob-root");
    let outside = tmp("glob-out");
    let secret = outside.join("secret.mkv");
    let bytes = b"GLOB-SECRET";
    fs::write(&secret, bytes).unwrap();

    // Windows forbids `*`/`?` in filenames; record only what the FS accepts.
    let mut created: Vec<&str> = Vec::new();
    for name in ["*.mkv", "?.mkv", "[abc].mkv", "movie*.mkv"] {
        if fs::write(root.join(name), b"x").is_ok() {
            created.push(name);
        }
    }

    let glob_escape = root
        .join("*")
        .join("..")
        .join("..")
        .join(outside.file_name().unwrap())
        .join("secret.mkv");
    assert!(
        !within(&root, &glob_escape),
        "glob path resolved outside root"
    );

    let out = scan(&root);
    assert_eq!(
        out.files.len(),
        created.len(),
        "glob names were not literal"
    );
    for name in &created {
        assert!(
            out.files.iter().any(|f| f.path.ends_with(name)),
            "wildcard name was not kept literal: {name}"
        );
    }
    assert_not_indexed(&out, "secret.mkv");
    assert_outside_untouched(&secret, bytes);
    cleanup(&root, &outside);
}

#[test]
fn cleanup_cannot_escape_the_managed_root() {
    let root = tmp("clean-root");
    let outside = tmp("clean-out");
    let keep = outside.join("keep.mkv");
    let bytes = b"CLEANUP-KEEP";
    fs::write(&keep, bytes).unwrap();

    // A cleanup pass that followed this path would delete an outside file.
    let escape = root
        .join("..")
        .join(outside.file_name().unwrap())
        .join("keep.mkv");
    assert!(!within(&root, &escape), "cleanup target escaped the root");

    for _ in 0..2 {
        let out = scan(&root);
        assert_not_indexed(&out, "keep.mkv");
        assert_outside_untouched(&keep, bytes);
    }
    cleanup(&root, &outside);
}
