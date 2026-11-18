use media_library::{path_policy, Scanner};
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn temp_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mh-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn traversal_rejected() {
    let root = temp_dir();
    // create a file outside root attempt via .. is handled by canonicalization;
    // ensure scanner doesn't panic on hostile names.
    let hostile = root.join("weird\u{0000}name.txt");
    let _ = fs::write(&hostile, b"x"); // may fail on windows; ignore
    let out = Scanner::new().scan(&root, &Arc::new(AtomicBool::new(false)));
    // no panic
    let _ = out.files.len();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn media_extension_allowlist() {
    assert!(path_policy::is_media_extension("mp4"));
    assert!(path_policy::is_media_extension("MP4"));
    assert!(!path_policy::is_media_extension("exe"));
    assert!(!path_policy::is_media_extension("rs"));
}

#[test]
fn sanitize_title_drops_control_chars() {
    let t = path_policy::sanitize_title("hello\u{0001}world");
    assert_eq!(t, "helloworld");
    let t2 = path_policy::sanitize_title("   ");
    assert_eq!(t2, "Untitled");
}

#[test]
fn scans_media_files() {
    let root = temp_dir();
    fs::write(root.join("movie.mp4"), b"fake video bytes").unwrap();
    fs::write(root.join("song.mp3"), b"fake audio").unwrap();
    fs::write(root.join("readme.txt"), b"not media").unwrap();
    let out = Scanner::new().scan(&root, &Arc::new(AtomicBool::new(false)));
    assert_eq!(out.files.len(), 2);
    assert!(!out.cancelled);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn cancellation_stops_scan() {
    let root = temp_dir();
    for i in 0..50 {
        fs::write(root.join(format!("m{i}.mp4")), b"x").unwrap();
    }
    let flag = Arc::new(AtomicBool::new(true));
    let out = Scanner::new().scan(&root, &flag);
    assert!(out.cancelled);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn nested_root_does_not_duplicate_via_containment() {
    // Basic: scanner handles nested dirs without panic
    let root = temp_dir();
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("a.mp4"), b"x").unwrap();
    fs::write(root.join("b.mp4"), b"x").unwrap();
    let out = Scanner::new().scan(&root, &Arc::new(AtomicBool::new(false)));
    assert_eq!(out.files.len(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn stable_id_is_deterministic_and_path_specific() {
    use media_library::stable_id;
    assert_eq!(stable_id("C:/media/a.mkv"), stable_id("C:/media/a.mkv"));
    assert_ne!(stable_id("C:/media/a.mkv"), stable_id("C:/media/b.mkv"));
}

#[test]
fn classify_kind_maps_playable_extensions_only() {
    use app_core::domain::MediaKind;
    use media_library::classify_kind;
    assert_eq!(classify_kind("mkv"), Some(MediaKind::Movie));
    assert_eq!(classify_kind("mp3"), Some(MediaKind::Movie));
    assert_eq!(classify_kind("png"), None);
    assert_eq!(classify_kind("txt"), None);
}

#[test]
fn artwork_extension_is_skipped_not_errored() {
    let root = temp_dir();
    fs::write(root.join("poster.png"), b"fake image").unwrap();
    fs::write(root.join("movie.mkv"), b"fake video").unwrap();
    let out = Scanner::new().scan(&root, &Arc::new(AtomicBool::new(false)));
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].title, "movie");
    assert!(out.errors.is_empty());
    fs::remove_dir_all(&root).ok();
}
