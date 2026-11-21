//! Optional TMDB provider: opt-in gate, injection, sanitisation, cache and
//! offline degradation. All fetch calls are fakes — no network is used.

use media_metadata::cache::Cache;
use media_metadata::tmdb::{
    classify_image, too_large, ArtworkCache, FetchFn, TmdbOptions, TmdbProvider, TMDB_ATTRIBUTION,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const GOOD_JSON: &str = r#"{"results":[{"title":"Arrival","release_date":"2016-11-11","overview":"A linguist is recruited.","genres":[{"name":"Science Fiction"},{"name":"Drama"}],"poster_path":"/abc123.jpg"}]}"#;

const HOSTILE_JSON: &str = r#"{"results":[{"title":"Ti\u0000tle <b>x</b>","release_date":"2016-11-11","overview":"a\u0001b   c","genres":[{"name":"G\u0002e"}],"poster_path":"/../../etc/passwd"}]}"#;

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(name: &str) -> PathBuf {
    let id = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("mh-tmdb-{name}-{}-{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn fetch_ok(calls: Arc<AtomicUsize>, json: &'static str) -> Box<FetchFn> {
    Box::new(move |_title, _year, _key| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(json.to_string())
    })
}

fn fetch_err(calls: Arc<AtomicUsize>) -> Box<FetchFn> {
    Box::new(move |_title, _year, _key| {
        calls.fetch_add(1, Ordering::SeqCst);
        Err("offline".to_string())
    })
}

fn options(enabled: bool, key: Option<&str>, name: &str) -> TmdbOptions {
    TmdbOptions {
        enabled,
        api_key: key.map(str::to_string),
        cache_dir: temp_dir(name),
        ..Default::default()
    }
}

#[test]
fn disabled_provider_makes_zero_requests_and_returns_none() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(false, Some("test-key"), "disabled"),
        fetch_ok(calls.clone(), GOOD_JSON),
    );
    assert!(provider.lookup("Arrival", Some(2016)).is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn enabled_without_key_fails_closed_without_a_request() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(true, None, "no-key"),
        fetch_ok(calls.clone(), GOOD_JSON),
    );
    assert!(provider.lookup("Arrival", Some(2016)).is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn injected_fetch_returns_a_parsed_record() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(true, Some("test-key"), "happy"),
        fetch_ok(calls.clone(), GOOD_JSON),
    );
    let record = provider.lookup("Arrival", Some(2016)).expect("a record");
    assert_eq!(record.title.as_deref(), Some("Arrival"));
    assert_eq!(record.year, Some(2016));
    assert_eq!(record.synopsis.as_deref(), Some("A linguist is recruited."));
    assert_eq!(
        record.genres,
        vec!["Science Fiction".to_string(), "Drama".to_string()]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn hostile_strings_and_poster_paths_are_sanitised() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(true, Some("test-key"), "hostile"),
        fetch_ok(calls, HOSTILE_JSON),
    );
    let found = provider.lookup_full("hostile", None).expect("a record");
    let title = found.record.title.as_ref().expect("title");
    assert_eq!(title, "Title <b>x</b>");
    assert!(!title.chars().any(char::is_control));
    assert_eq!(found.record.synopsis.as_deref(), Some("ab c"));
    assert!(found
        .record
        .genres
        .iter()
        .all(|genre| !genre.chars().any(char::is_control)));
    assert_eq!(found.poster_path, None);
    assert_eq!(found.attribution(), TMDB_ATTRIBUTION);
}

#[test]
fn fresh_cache_hit_makes_no_second_request() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(true, Some("test-key"), "cache-hit"),
        fetch_ok(calls.clone(), GOOD_JSON),
    );
    assert!(provider.lookup("Arrival", Some(2016)).is_some());
    assert!(provider.lookup("Arrival", Some(2016)).is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn fetch_failure_degrades_to_none_without_panic() {
    let calls = counter();
    let provider = TmdbProvider::with_fetch(
        options(true, Some("test-key"), "offline"),
        fetch_err(calls.clone()),
    );
    assert!(provider.lookup("Arrival", Some(2016)).is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn cache_evicts_oldest_when_over_the_byte_cap() {
    let dir = temp_dir("eviction");
    let cache = Cache::new(dir.clone(), 250);
    cache.put("a", &[b'a'; 100]).expect("write a");
    cache.put("b", &[b'b'; 100]).expect("write b");
    cache
        .put("c", &[b'c'; 100])
        .expect("write c triggers eviction");

    let mut total = 0u64;
    let mut count = 0usize;
    for entry in fs::read_dir(&dir).expect("read cache dir") {
        let entry = entry.expect("entry");
        let meta = entry.metadata().expect("metadata");
        if meta.is_file() {
            total += meta.len();
            count += 1;
        }
    }
    assert_eq!(count, 2);
    assert!(total <= 250);
}

#[test]
fn cache_entry_expires_after_thirty_days() {
    let dir = temp_dir("freshness");
    let cache = Cache::new(dir.clone(), 1_000_000);
    cache.put("k", b"value").expect("write entry");
    assert_eq!(cache.get("k"), Some(b"value".to_vec()));

    let path = fs::read_dir(&dir)
        .expect("read cache dir")
        .find_map(|entry| {
            let entry = entry.expect("entry");
            entry
                .metadata()
                .expect("metadata")
                .is_file()
                .then(|| entry.path())
        })
        .expect("cached file");
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open cached file for write")
        .set_modified(SystemTime::now() - Duration::from_secs(31 * 24 * 60 * 60))
        .expect("set old mtime");

    assert!(cache.get("k").is_none());
}

#[test]
fn artwork_key_is_opaque_sha256_hex() {
    let key = media_metadata::tmdb::artwork_key("/abc123.jpg");
    assert_eq!(key.len(), 64);
    assert!(key
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(key, media_metadata::tmdb::artwork_key("/other.jpg"));
}

#[test]
fn image_bytes_are_type_checked_and_capped() {
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    assert_eq!(classify_image(&jpeg), Some("image/jpeg"));
    assert_eq!(classify_image(&[0x89, 0x50, 0x4E, 0x47]), Some("image/png"));
    assert_eq!(classify_image(b"<script>alert(1)</script>"), None);
    assert!(too_large(2 * 1024 * 1024 + 1));
}

#[test]
fn artwork_cache_stores_and_evicts_oldest_first() {
    let dir = std::env::temp_dir().join(format!("mh-art-{}", uuid::Uuid::new_v4()));
    let cache = ArtworkCache::new(dir.clone(), 8); // tiny cap forces eviction
    let fake_jpeg = b"\xFF\xD8\xFF\xE0\x00"; // 5 bytes, valid JPEG magic header
    cache.put("aaa", fake_jpeg, "image/jpeg").unwrap();
    cache.put("bbb", fake_jpeg, "image/jpeg").unwrap();
    assert!(cache.get("aaa").is_none()); // oldest evicted
    let (bytes, kind) = cache.get("bbb").expect("kept entry");
    assert_eq!(bytes, fake_jpeg);
    assert_eq!(kind, "image/jpeg");
    let _ = std::fs::remove_dir_all(dir);
}
