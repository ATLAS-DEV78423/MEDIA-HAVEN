//! Offline NFO reading: the happy path plus the refusals that keep a hostile
//! sidecar from costing more than one skipped field.

use media_metadata::{parse, read_beside, NfoError};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mh-nfo-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write file");
    path
}

const MOVIE_NFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<movie>
  <title>Arrival &amp; the Heptapods</title>
  <year>2016</year>
  <plot>
     A linguist   is recruited to translate.
  </plot>
  <runtime>116</runtime>
  <genre>Science Fiction</genre>
  <genre>Drama</genre>
  <genre>science fiction</genre>
  <director>Denis Villeneuve</director>
</movie>
"#;

#[test]
fn reads_fields_from_the_sidecar_beside_the_media_file() {
    let dir = temp_dir("fields");
    let media = write(&dir, "Arrival.mkv", b"not a real video");
    write(&dir, "Arrival.nfo", MOVIE_NFO.as_bytes());

    let record = read_beside(&media)
        .expect("read sidecar")
        .expect("sidecar present");
    assert_eq!(record.title.as_deref(), Some("Arrival & the Heptapods"));
    assert_eq!(record.year, Some(2016));
    assert_eq!(
        record.synopsis.as_deref(),
        Some("A linguist is recruited to translate.")
    );
    assert_eq!(record.runtime_minutes, Some(116));
    // Duplicate genres collapse case-insensitively and keep document order.
    assert_eq!(record.genres, vec!["Science Fiction", "Drama"]);
}

#[test]
fn the_file_stem_wins_over_a_shared_movie_nfo() {
    let dir = temp_dir("precedence");
    let media = write(&dir, "Heat.mkv", b"x");
    write(
        &dir,
        "Heat.nfo",
        b"<movie><title>Heat (sidecar)</title></movie>",
    );
    write(
        &dir,
        "movie.nfo",
        b"<movie><title>Wrong film</title></movie>",
    );

    let record = read_beside(&media).unwrap().unwrap();
    assert_eq!(record.title.as_deref(), Some("Heat (sidecar)"));
}

#[test]
fn a_file_without_a_sidecar_is_not_an_error() {
    let dir = temp_dir("none");
    let media = write(&dir, "Bare.mkv", b"x");
    assert_eq!(read_beside(&media).expect("no sidecar"), None);
}

#[test]
fn a_dtd_or_entity_declaration_is_refused() {
    let dir = temp_dir("doctype");
    let media = write(&dir, "Xxe.mkv", b"x");
    write(
        &dir,
        "Xxe.nfo",
        br#"<?xml version="1.0"?>
<!DOCTYPE movie SYSTEM "file:///c:/windows/win.ini">
<movie><title>Xxe</title></movie>"#,
    );

    assert_eq!(
        read_beside(&media).expect_err("doctype refused"),
        NfoError::DoctypeRefused
    );

    let entity =
        b"<!ENTITY lol SYSTEM \"file:///c:/windows/win.ini\">\n<movie><title>&lol;</title></movie>";
    assert_eq!(
        parse(entity).expect_err("entity refused"),
        NfoError::DoctypeRefused
    );
}

#[test]
fn an_oversized_sidecar_is_refused_without_parsing_it() {
    let dir = temp_dir("large");
    let media = write(&dir, "Big.mkv", b"x");
    write(&dir, "Big.nfo", &vec![b'a'; 300 * 1024]);

    assert_eq!(
        read_beside(&media).expect_err("too large"),
        NfoError::TooLarge
    );
}

#[test]
fn control_characters_are_refused() {
    let body = b"<movie><title>bad\x00title</title></movie>";
    assert_eq!(
        parse(body).expect_err("control characters"),
        NfoError::ControlCharacters
    );
}

#[test]
fn an_unterminated_element_is_malformed_not_a_panic() {
    let body = b"<movie><title>Unclosed</movie>";
    match parse(body) {
        Err(NfoError::Malformed(_)) => {}
        other => panic!("expected malformed, got {other:?}"),
    }
}

#[test]
fn long_values_are_capped_and_blank_values_are_dropped() {
    let long_plot = "x".repeat(5000);
    let body = format!("<movie><title></title><year>9999</year><plot>{long_plot}</plot></movie>");
    let record = parse(body.as_bytes()).expect("parse");

    assert_eq!(
        record.title, None,
        "a blank title must not blank the filename"
    );
    assert_eq!(record.year, None, "an impossible year is dropped");
    assert_eq!(
        record.synopsis.map(|s| s.chars().count()),
        Some(4000),
        "synopsis is capped"
    );
}
