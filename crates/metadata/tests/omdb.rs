//! Optional OMDb provider: opt-in gate, fail-closed key check, sanitisation,
//! cache TTL/eviction and zero-request behaviour. Every fetch is a fake — no
//! network or live key is used.

use app_core::domain::{ImdbOptInSettings, IMDB_OPT_IN_TEXT};
use media_metadata::cache::RatingCache;
use media_metadata::omdb::{MemoryOmdbFetch, OmdbError, OmdbOptions, OmdbProvider, API_KEY_ENV};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

fn ratings() -> Vec<(String, String)> {
    vec![
        ("tt0111161".to_string(), "R".to_string()),
        ("tt0468569".to_string(), "PG-13".to_string()),
    ]
}

fn enabled_provider(fetch: Arc<MemoryOmdbFetch>, key: Option<&str>) -> OmdbProvider {
    OmdbProvider::with_fetch(
        OmdbOptions {
            enabled: true,
            api_key: key.map(str::to_string),
            ..Default::default()
        },
        Box::new(fetch),
    )
}

#[test]
fn disabled_provider_makes_zero_requests() {
    let fetch = Arc::new(MemoryOmdbFetch::new(ratings()));
    let provider = OmdbProvider::with_fetch(OmdbOptions::default(), Box::new(fetch.clone()));
    assert!(!provider.options().enabled);
    assert_eq!(
        provider.age_rating_for("tt0111161"),
        Err(OmdbError::Unavailable)
    );
    assert_eq!(fetch.call_count(), 0);
}

#[test]
fn missing_or_blank_key_fails_closed_without_a_request() {
    for key in [None, Some(""), Some("   ")] {
        let fetch = Arc::new(MemoryOmdbFetch::new(ratings()));
        let provider = enabled_provider(fetch.clone(), key);
        assert_eq!(
            provider.age_rating_for("tt0111161"),
            Err(OmdbError::Unavailable)
        );
        assert_eq!(fetch.call_count(), 0);
    }
}

#[test]
fn default_provider_fails_closed() {
    let provider = OmdbProvider::new(OmdbOptions::default());
    assert_eq!(
        provider.age_rating_for("tt0111161"),
        Err(OmdbError::Unavailable)
    );
}

#[test]
fn enabled_provider_returns_a_sanitised_certification() {
    let fetch = Arc::new(MemoryOmdbFetch::new(vec![(
        "tt0111161".to_string(),
        "  PG\u{0}-13 \u{1}".to_string(),
    )]));
    let provider = enabled_provider(fetch.clone(), Some("test-key"));
    let rating = provider
        .age_rating_for("tt0111161")
        .expect("a rating")
        .expect("a certification");
    assert_eq!(rating, "PG-13");
    assert!(!rating.chars().any(char::is_control));
    assert_eq!(fetch.call_count(), 1);
}

#[test]
fn unknown_id_returns_none_not_an_invented_rating() {
    let fetch = Arc::new(MemoryOmdbFetch::new(ratings()));
    let provider = enabled_provider(fetch.clone(), Some("test-key"));
    assert_eq!(provider.age_rating_for("tt0000000"), Ok(None));
    assert_eq!(fetch.call_count(), 1);
}

#[test]
fn path_like_or_malformed_id_is_never_sent() {
    let fetch = Arc::new(MemoryOmdbFetch::new(ratings()));
    let provider = enabled_provider(fetch.clone(), Some("test-key"));
    for bad in ["../../etc/passwd", "C:\\secret", "not-an-id", ""] {
        assert_eq!(provider.age_rating_for(bad), Ok(None));
    }
    assert_eq!(fetch.call_count(), 0);
}

#[test]
fn fresh_cache_hit_makes_no_second_request() {
    let fetch = Arc::new(MemoryOmdbFetch::new(ratings()));
    let provider = enabled_provider(fetch.clone(), Some("test-key"));
    assert_eq!(
        provider.age_rating_for("tt0111161").expect("first"),
        Some("R".to_string())
    );
    assert_eq!(
        provider.age_rating_for("tt0111161").expect("second"),
        Some("R".to_string())
    );
    assert_eq!(fetch.call_count(), 1);
}

#[test]
fn rating_cache_expires_after_thirty_days() {
    let mut cache = RatingCache::new(1024);
    let old = SystemTime::now() - Duration::from_secs(31 * 24 * 60 * 60);
    cache.put_at("tt0000001", "PG-13", old);
    assert_eq!(cache.get("tt0000001"), None);
    cache.put("tt0000002", "R");
    assert_eq!(cache.get("tt0000002"), Some("R".to_string()));
}

#[test]
fn rating_cache_evicts_oldest_when_over_the_byte_cap() {
    let mut cache = RatingCache::new(8);
    let now = SystemTime::now();
    cache.put_at("tt0000001", "PG13", now - Duration::from_secs(180));
    cache.put_at("tt0000002", "PG13", now - Duration::from_secs(120));
    cache.put_at("tt0000003", "PG13", now - Duration::from_secs(60));
    assert_eq!(cache.get("tt0000001"), None);
    assert_eq!(cache.get("tt0000002"), Some("PG13".to_string()));
    assert_eq!(cache.get("tt0000003"), Some("PG13".to_string()));
}

#[test]
fn opt_in_defaults_off_and_text_names_what_is_sent_and_to_whom() {
    let settings = ImdbOptInSettings::default();
    assert!(!settings.imdb_allowed());
    assert!(!settings.omdb_allowed());
    assert!(IMDB_OPT_IN_TEXT.contains("title/year"));
    assert!(IMDB_OPT_IN_TEXT.contains("IMDb id"));
    assert!(IMDB_OPT_IN_TEXT.contains("IMDb datasets"));
    assert!(IMDB_OPT_IN_TEXT.contains("OMDb"));
    // The key is sourced from the environment/secret store, never a DTO.
    assert_eq!(API_KEY_ENV, "MEDIAHAVEN_OMDB_API_KEY");
}
