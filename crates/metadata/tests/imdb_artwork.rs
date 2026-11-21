//! Keyless IMDb artwork provider. All fetch calls are fakes — no network is used.

use media_metadata::imdb_artwork::{validate_image_url, ImdbArtworkProvider};

const FAKE_URL: &str = "https://m.media-amazon.com/images/M/MV5Btest._V1_.jpg";

#[test]
fn passes_title_and_year_to_fetch_and_returns_the_url() {
    // The injectable seam returns an image URL (its JSON parsing is tested
    // offline in the provider's unit tests); no network is used.
    let provider = ImdbArtworkProvider::with_fetch(Box::new(|title, year| {
        assert_eq!(title, "Arrival");
        assert_eq!(year, Some(2016));
        Ok(FAKE_URL.to_string())
    }));
    assert_eq!(
        provider.poster_url("Arrival", Some(2016)).as_deref(),
        Some(FAKE_URL)
    );
}

#[test]
fn rejects_non_imdb_image_hosts() {
    assert!(validate_image_url(FAKE_URL));
    assert!(!validate_image_url("https://evil.example/a.jpg"));
    assert!(!validate_image_url(
        "http://m.media-amazon.com/images/M/a.jpg"
    ));
    assert!(!validate_image_url(
        "https://m.media-amazon.com/../etc/x.jpg"
    ));
    assert!(!validate_image_url("javascript:alert(1)"));
    let long = format!("https://m.media-amazon.com/images/M/{}", "a".repeat(600));
    assert!(!validate_image_url(&long));
    assert!(!validate_image_url(
        "https://m.media-amazon.com/images/M/a\nb.jpg"
    ));
}

#[test]
fn poster_url_caches_identical_lookups() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let provider = ImdbArtworkProvider::with_fetch(Box::new(move |title, _| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(format!(
            "https://m.media-amazon.com/images/M/{title}._V1_.jpg"
        ))
    }));

    assert!(provider.poster_url("Arrival", Some(2016)).is_some());
    assert!(provider.poster_url("Arrival", Some(2016)).is_some());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "warm shelf must not refetch"
    );

    assert!(provider.poster_url("Alien", Some(1979)).is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 2, "new title must fetch once");
}

#[test]
fn provider_fails_closed_on_fetch_error_or_bad_url() {
    let bad = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Ok("https://evil.example/x".into())));
    assert_eq!(bad.poster_url("x", None), None);
    let err = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Err("boom".into())));
    assert_eq!(err.poster_url("x", None), None);
}
