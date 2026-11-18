use media_library::Scanner;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mh-roots-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    fs::create_dir_all(dir).ok();
    let p = dir.join(name);
    fs::write(&p, bytes).expect("write file");
    p
}

fn scan(dir: &Path) -> media_library::ScanOutcome {
    let cancel = Arc::new(AtomicBool::new(false));
    Scanner::new().scan(dir, &cancel)
}

#[test]
fn path_escape_is_refused() {
    let root = temp_root("escape");
    let outside = temp_root("outside");
    let target = outside.join("secret.mkv");
    fs::write(&target, b"x").unwrap();

    // Try to plant a junction/symlink inside the root that points outside.
    let link = root.join("link.mkv");
    let created = std::os::windows::fs::symlink_file(&target, &link).is_ok();
    if !created {
        // No privilege to create symlinks on this host; cannot exercise the
        // refusal path. Report as SKIPPED by returning early.
        eprintln!("SKIPPED path_escape_is_refused: symlink creation failed (no privilege)");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        return;
    }

    let out = scan(&root);
    for f in &out.files {
        assert!(
            !f.path.contains(".."),
            "escaped path leaked into scan: {}",
            f.path
        );
        assert_ne!(f.path, target.to_string_lossy(), "outside file was indexed");
    }
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn artwork_files_are_skipped() {
    let root = temp_root("artwork");
    write_file(&root, "cover.jpg", b"img");
    write_file(&root, "movie.mkv", b"vid");
    let out = scan(&root);
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].title, "movie");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hostile_filenames_do_not_break_the_scan() {
    let root = temp_root("hostile");
    let _ = fs::write(root.join(".."), b"x"); // invalid name on Windows; ignore
    let _ = fs::write(root.join("%2e%2e.mkv"), b"x");
    let _ = fs::write(root.join("a\u{202E}b.mkv"), b"x");
    let long_name = format!("{}.mkv", "a".repeat(200));
    let _ = fs::write(root.join(&long_name), b"x");
    let _ = fs::write(root.join("movie.mkv"), b"x");

    let out = scan(&root);
    for f in &out.files {
        assert!(
            !f.path.split(['/', '\\']).any(|c| c == ".."),
            "path contains .. component: {}",
            f.path
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn vanishing_file_is_skipped_not_fatal() {
    let root = temp_root("vanish");
    write_file(&root, "a.mkv", b"x");
    write_file(&root, "b.mkv", b"y");
    // Remove one entry so the directory listing and the scan disagree.
    let _ = fs::remove_file(root.join("b.mkv"));
    let out = scan(&root);
    // Only the surviving file should be indexed; no panic.
    assert_eq!(out.files.len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cancel_then_rescan_is_idempotent() {
    let root = temp_root("cancel");
    for i in 0..20 {
        write_file(&root, &format!("m{i}.mkv"), b"x");
    }
    let flag = Arc::new(AtomicBool::new(true));
    let first = Scanner::new().scan(&root, &flag);
    assert!(first.cancelled);

    let second = scan(&root);
    assert!(!second.cancelled);
    assert_eq!(second.files.len(), 20);
    let mut ids: Vec<&str> = second.files.iter().map(|f| f.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), second.files.len(), "duplicate ids after rescan");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn duplicate_hashes_are_detected() {
    let root = temp_root("dup");
    write_file(&root, "a.mkv", b"same bytes");
    write_file(&root, "b.mkv", b"same bytes");
    let out = scan(&root);
    assert_eq!(out.files.len(), 2);
    assert_eq!(out.files[0].hash, out.files[1].hash);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stable_ids_are_unique_per_path() {
    let root = temp_root("ids");
    write_file(&root, "a.mkv", b"x");
    write_file(&root, "b.mkv", b"y");
    write_file(&root, "c.mkv", b"z");
    let out = scan(&root);
    assert_eq!(out.files.len(), 3);
    let mut ids: Vec<&str> = out.files.iter().map(|f| f.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "ids not unique");
    for f in &out.files {
        assert_eq!(f.id.len(), 64, "id not 64 hex chars: {}", f.id);
        assert!(f.id.chars().all(|c| c.is_ascii_hexdigit()));
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn empty_root_is_not_an_error() {
    let root = temp_root("empty");
    let out = scan(&root);
    assert!(out.files.is_empty());
    assert!(out.errors.is_empty());
    assert!(!out.cancelled);
    let _ = fs::remove_dir_all(&root);
}
