//! Display-only discovery provider: fake fetches only, no network.
//!
//! Note: the brief's first test used a one-argument `with_fetch`, but the
//! authoritative implementation snippet (and the `TmdbProvider::with_fetch`
//! house pattern) takes `(options, fetch)`. The test supplies options so both
//! the brief's contract and the house convention are honoured.

use media_metadata::discover::{discover_url, CatalogKind, DiscoverOptions, DiscoverProvider};

const MOVIE_JSON: &str = r#"{"page":1,"total_results":2,"results":[
  {"id":1,"title":"Real Film","release_date":"2020-05-01","overview":"A story.","poster_path":"/p1.jpg","vote_average":7.2},
  {"id":2,"title":"Bad <b>title</b>\u0001","release_date":"not-a-date","overview":"x","poster_path":"http://evil/x","vote_average":99}]}"#;

fn movie_options() -> DiscoverOptions {
    let dir = std::env::temp_dir().join(format!("mh-discover-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    DiscoverOptions {
        enabled: true,
        api_key: Some("test-key".to_string()),
        cache_dir: dir,
    }
}

#[test]
fn parses_and_sanitises_movie_page() {
    let provider = DiscoverProvider::with_fetch(
        movie_options(),
        Box::new(|_, _, _| Ok(MOVIE_JSON.to_string())),
    );
    let entries = provider.discover(CatalogKind::Movie, 1);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Real Film");
    assert_eq!(entries[0].year, Some(2020));
    assert_eq!(entries[0].poster_path.as_deref(), Some("/p1.jpg"));
    assert!(entries[1].title.chars().all(|c| !c.is_control()));
    assert_eq!(entries[1].poster_path, None); // non-CDN path rejected
}

#[test]
fn disabled_provider_makes_no_fetch() {
    let provider = DiscoverProvider::with_options(DiscoverOptions {
        enabled: false,
        api_key: None,
        cache_dir: std::env::temp_dir().join("mh-discover-disabled-test"),
    });
    assert!(provider.discover(CatalogKind::Movie, 1).is_empty());
}

#[test]
fn anime_uses_japanese_animation_filters() {
    let url = discover_url(CatalogKind::Anime, 2);
    assert!(url.contains("with_genres=16"));
    assert!(url.contains("with_original_language=ja"));
}
