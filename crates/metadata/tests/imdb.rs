//! Offline IMDb provider: TSV parsing, merging, sanitisation, opt-in gate and
//! fail-closed behaviour. Every fetch is an in-memory fake — no network is used.

use media_metadata::imdb::{
    merge, parse_basics, parse_ratings, ImdbOptions, ImdbProvider, MemoryFetch, IMDB_ATTRIBUTION,
};
use std::sync::Arc;

const BASICS: &str = "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0111161\tmovie\tThe Shawshank Redemption\tThe Shawshank Redemption\t0\t1994\t\\N\t142\tDrama\n\
tt0468569\tmovie\tThe Dark Knight\tThe Dark Knight\t0\t2008\t\\N\t152\tAction,Crime,Drama\n\
tt0903747\ttvSeries\tBreaking Bad\tBreaking Bad\t0\t2008\t2013\t\\N\tCrime,Drama\n";

const RATINGS: &str = "tconst\taverageRating\tnumVotes\n\
tt0111161\t9.3\t2500000\n\
tt0468569\t9.0\t\\N\n\
tt9999999\t8.0\t100\n";

const BASICS_NULL: &str = "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0000001\tmovie\tNo Year\tNo Year\t0\t\\N\t\\N\t\\N\t\\N\n";

const RATINGS_NULL: &str = "tconst\taverageRating\tnumVotes\n\
tt0000001\t\\N\t\\N\n";

const BASICS_HOSTILE: &str = "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0000002\tmovie\tBad\u{0}  Ti\u{1}tle \u{2}Here\tBad Title Here\t0\t2010\t\\N\t90\tAction,G\u{3}enre\n";

const BASICS_TYPES: &str = "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0000010\ttvEpisode\tPilot\tPilot\t0\t2010\t\\N\t\\N\tDrama\n\
tt0000011\tvideoGame\tGame\tGame\t0\t2010\t\\N\t\\N\tAction\n\
tt0000012\tmovie\tA Movie\tA Movie\t0\t2011\t\\N\t90\tDrama\n";

fn enabled_provider() -> ImdbProvider {
    let fetch = Arc::new(MemoryFetch::new(
        parse_basics(BASICS),
        parse_ratings(RATINGS),
    ));
    ImdbProvider::with_fetch(ImdbOptions { enabled: true }, Box::new(fetch))
}

#[test]
fn valid_rows_are_parsed() {
    let rows = parse_basics(BASICS);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].imdb_id, "tt0111161");
    assert_eq!(rows[0].title, "The Shawshank Redemption");
    assert_eq!(rows[0].year, Some(1994));
    assert_eq!(rows[0].genres, vec!["Drama".to_string()]);
    assert_eq!(rows[1].genres, vec!["Action", "Crime", "Drama"]);
    assert_eq!(rows[2].title, "Breaking Bad");
    assert_eq!(rows[2].year, Some(2008));
}

#[test]
fn ratings_rows_are_parsed() {
    let rows = parse_ratings(RATINGS);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].imdb_id, "tt0111161");
    assert_eq!(rows[0].rating, Some(9.3));
    assert_eq!(rows[0].votes, Some(2_500_000));
    assert_eq!(rows[2].imdb_id, "tt9999999");
}

#[test]
fn null_tokens_become_none() {
    let basics = parse_basics(BASICS_NULL);
    assert_eq!(basics.len(), 1);
    assert_eq!(basics[0].year, None);
    assert!(basics[0].genres.is_empty());

    let ratings = parse_ratings(RATINGS_NULL);
    assert_eq!(ratings.len(), 1);
    assert_eq!(ratings[0].rating, None);
    assert_eq!(ratings[0].votes, None);
}

#[test]
fn hostile_title_is_sanitised() {
    let rows = parse_basics(BASICS_HOSTILE);
    assert_eq!(rows.len(), 1);
    let title = &rows[0].title;
    assert_eq!(title, "Bad Title Here");
    assert!(!title.chars().any(char::is_control));
    assert_eq!(rows[0].genres, vec!["Action", "Genre"]);
    assert!(rows[0]
        .genres
        .iter()
        .all(|genre| !genre.chars().any(char::is_control)));
}

#[test]
fn non_movie_and_show_types_are_skipped() {
    let rows = parse_basics(BASICS_TYPES);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].imdb_id, "tt0000012");
}

#[test]
fn merge_attaches_ratings_by_id() {
    let merged = merge(parse_basics(BASICS), parse_ratings(RATINGS));
    assert_eq!(merged.len(), 3);
    let first = merged.iter().find(|r| r.imdb_id == "tt0111161").unwrap();
    assert_eq!(first.rating, Some(9.3));
    assert_eq!(first.votes, Some(2_500_000));
    let second = merged.iter().find(|r| r.imdb_id == "tt0468569").unwrap();
    assert_eq!(second.rating, Some(9.0));
    assert_eq!(second.votes, None);
    let unranked = merged.iter().find(|r| r.imdb_id == "tt0903747").unwrap();
    assert_eq!(unranked.rating, None);
    assert!(!merged.iter().any(|r| r.imdb_id == "tt9999999"));
}

#[test]
fn unknown_id_returns_none() {
    let provider = enabled_provider();
    assert!(provider.by_id("tt0000000").is_none());
    assert!(provider.by_id("not-an-id").is_none());
}

#[test]
fn enabled_provider_returns_merged_records_and_attribution() {
    let provider = enabled_provider();
    let records = provider.records().expect("records when enabled");
    assert_eq!(records.len(), 3);
    assert_eq!(provider.attribution(), IMDB_ATTRIBUTION);
    assert_eq!(IMDB_ATTRIBUTION, "Ratings and age ratings: IMDb / OMDb");
}

#[test]
fn disabled_provider_makes_no_fetch_calls() {
    let fetch = Arc::new(MemoryFetch::new(
        parse_basics(BASICS),
        parse_ratings(RATINGS),
    ));
    let provider = ImdbProvider::with_fetch(ImdbOptions::default(), Box::new(fetch.clone()));
    assert!(!provider.options().enabled);
    assert!(provider.records().is_none());
    assert!(provider.by_id("tt0111161").is_none());
    assert_eq!(fetch.call_count(), 0);
}

#[test]
fn default_provider_fails_closed() {
    let provider = ImdbProvider::new(ImdbOptions::default());
    assert!(provider.records().is_none());
    assert!(provider.by_id("tt0111161").is_none());
}
