//! Keyless IMDb poster lookup. Only a title and year are sent to IMDb's public
//! suggestion endpoint; no library path or content is included.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::tmdb::{classify_image, MAX_ARTWORK_BYTES};

pub type ArtworkFetchFn = dyn Fn(&str, Option<i64>) -> Result<String, String> + Send + Sync;

const IMAGE_PREFIX: &str = "https://m.media-amazon.com/images/M/";
const URL_CACHE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const URL_CACHE_MAX: usize = 512;

/// Accept only IMDb's poster CDN over HTTPS. No traversal, no control chars.
pub fn validate_image_url(url: &str) -> bool {
    url.starts_with(IMAGE_PREFIX)
        && url.len() <= 512
        && !url.contains("..")
        && !url.chars().any(char::is_control)
}

pub struct ImdbArtworkProvider {
    fetch: Box<ArtworkFetchFn>,
    /// Bounded title/year -> validated URL cache. A warm shelf makes zero
    /// suggestion requests; each entry expires after 30 days.
    urls: Mutex<HashMap<String, (String, Instant)>>,
}

impl ImdbArtworkProvider {
    pub fn new() -> Self {
        Self::with_fetch(Box::new(imdb_search_fetch))
    }

    pub fn with_fetch(fetch: Box<ArtworkFetchFn>) -> Self {
        Self {
            fetch,
            urls: Mutex::new(HashMap::new()),
        }
    }

    /// Return an allowlisted image URL, or `None` on any failure.
    /// Cache hits are re-validated before use, so an entry can never become a
    /// disallowed URL without being refused.
    pub fn poster_url(&self, title: &str, year: Option<i64>) -> Option<String> {
        let key = format!("{title}|{year:?}");
        {
            let mut urls = self.urls.lock().ok()?;
            let cached = urls
                .get(&key)
                .map(|(url, stamped)| (url.clone(), stamped.elapsed() <= URL_CACHE_TTL));
            if let Some((url, fresh)) = cached {
                if fresh {
                    return validate_image_url(&url).then_some(url);
                }
                urls.remove(&key);
            }
        }

        let url = (self.fetch)(title, year).ok()?;
        if !validate_image_url(&url) {
            return None;
        }
        if let Ok(mut urls) = self.urls.lock() {
            // shortcut: clear-on-cap instead of LRU — a full cache drops every
            // warm entry once, then refills. Upgrade to LRU if that cost matters.
            if urls.len() >= URL_CACHE_MAX {
                urls.clear();
            }
            urls.insert(key, (url.clone(), Instant::now()));
        }
        Some(url)
    }
}

impl Default for ImdbArtworkProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn percent_encode(raw: &str) -> String {
    raw.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Pick a real title (`id` starts with `tt`), matching year and title first.
/// All errors are fixed, URL-free strings so no identifier can leak.
fn parse_suggestion(body: &str, title: &str, year: Option<i64>) -> Result<String, &'static str> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid IMDb response")?;
    let list = value
        .get("d")
        .and_then(Value::as_array)
        .ok_or("no IMDb results")?;
    let wanted = title.trim().to_ascii_lowercase();
    let is_title = |hit: &Value| {
        hit.get("id")
            .and_then(Value::as_str)
            .map(|id| id.starts_with("tt"))
            .unwrap_or(false)
    };
    let year_ok = |hit: &Value| {
        year.map(|y| hit.get("y").and_then(Value::as_i64) == Some(y))
            .unwrap_or(true)
    };
    let hit = list
        .iter()
        .find(|hit| {
            is_title(hit)
                && year_ok(hit)
                && hit
                    .get("l")
                    .and_then(Value::as_str)
                    .map(|l| l.to_ascii_lowercase() == wanted)
                    .unwrap_or(false)
        })
        .or_else(|| list.iter().find(|hit| is_title(hit) && year_ok(hit)))
        .ok_or("no IMDb title match")?;
    hit.get("i")
        .and_then(|image| image.get("imageUrl"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("IMDb title has no image")
}

/// First ASCII letter of a title, or `'a'` for digit-leading/non-ASCII titles.
/// IMDb's suggestion path is keyed by an ASCII letter; anything else 404s.
fn suggestion_letter(title: &str) -> char {
    title
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .unwrap_or('a')
        .to_ascii_lowercase()
}

/// Fetch the KEYLESS suggestion JSON and return only the image URL string.
///
/// The returned URL is UNVALIDATED by contract. Callers MUST run
/// `validate_image_url` before using it; prefer `ImdbArtworkProvider::poster_url`,
/// which validates on both cache hits and fresh lookups.
pub fn imdb_search_fetch(title: &str, year: Option<i64>) -> Result<String, String> {
    let clean = title.trim();
    if clean.is_empty() {
        return Err("empty title".to_string());
    }
    let letter = suggestion_letter(clean);
    let url = format!(
        "https://v2.sg.media-imdb.com/suggestion/{letter}/{}.json",
        percent_encode(clean)
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    let body = client
        .get(url)
        .send()
        .map_err(|_| "IMDb request failed".to_string())?
        .error_for_status()
        .map_err(|_| "IMDb request failed".to_string())?
        .text()
        .map_err(|_| "IMDb response unreadable".to_string())?;
    parse_suggestion(&body, clean, year).map_err(str::to_string)
}

/// Return the content type for real JPEG/PNG magic bytes, or `None` for
/// anything else — including non-images and bodies over `MAX_ARTWORK_BYTES`.
pub fn validate_image_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > MAX_ARTWORK_BYTES {
        return None;
    }
    classify_image(bytes)
}

/// Download only an allowlisted IMDb poster, bounded and type-checked.
pub fn fetch_image_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    if !validate_image_url(url) {
        return Err("image URL refused".to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|_| "image request failed".to_string())?
        .error_for_status()
        .map_err(|_| "image request failed".to_string())?
        .bytes()
        .map_err(|_| "image response unreadable".to_string())?;
    let kind = validate_image_bytes(&bytes).ok_or_else(|| "poster is not an image".to_string())?;
    Ok((bytes.to_vec(), kind.to_string()))
}

// shortcut: the IMDb suggestion endpoint is unofficial; a change in its JSON
// shape degrades to bundled art rather than breaking the view — upgrade only
// when an official keyless feed exists.

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_URL: &str = "https://m.media-amazon.com/images/M/MV5Btest._V1_.jpg";
    const SAMPLE: &str = r#"{"d":[
        {"id":"/star-wars","l":"Star Wars","y":1977,"i":{"imageUrl":"https://evil.example/not-a-title.jpg"}},
        {"id":"tt0086567","l":"Arrival","y":1984,"i":{"imageUrl":"https://m.media-amazon.com/images/M/MV5Bwrong-year._V1_.jpg"}},
        {"id":"tt2543164","l":"Arrival","y":2016,"i":{"imageUrl":"https://m.media-amazon.com/images/M/MV5Btest._V1_.jpg"}}
    ]}"#;

    #[test]
    fn picks_a_real_title_with_matching_year() {
        assert_eq!(
            parse_suggestion(SAMPLE, "Arrival", Some(2016)).unwrap(),
            FAKE_URL
        );
        // The non-title entry has a poster and a matching year, but must never win.
        assert_eq!(
            parse_suggestion(SAMPLE, "Star Wars", Some(1977)).unwrap_err(),
            "no IMDb title match"
        );
        assert_eq!(
            parse_suggestion(SAMPLE, "Arrival", None).unwrap(),
            "https://m.media-amazon.com/images/M/MV5Bwrong-year._V1_.jpg"
        );
    }

    #[test]
    fn parse_errors_are_fixed_and_url_free() {
        for body in [
            "not json",
            r#"{"other":[]}"#,
            r#"{"d":[]}"#,
            r#"{"d":[{"id":"tt1","l":"Arrival","y":2016}]}"#,
        ] {
            let err = parse_suggestion(body, "Arrival", Some(2016)).unwrap_err();
            assert!(!err.contains("http"));
            assert!(!err.contains("Arrival"));
        }
    }

    #[test]
    fn validate_image_bytes_accepts_jpeg_and_png_magic() {
        assert_eq!(
            validate_image_bytes(&[0xFF, 0xD8, 0xFF, 0x00]),
            Some("image/jpeg")
        );
        assert_eq!(
            validate_image_bytes(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]),
            Some("image/png")
        );
    }

    #[test]
    fn validate_image_bytes_rejects_non_images_and_oversize() {
        assert_eq!(
            validate_image_bytes(b"<html><script>alert(1)</script></html>"),
            None
        );
        let mut huge = vec![0xFF, 0xD8, 0xFF];
        huge.resize(MAX_ARTWORK_BYTES + 1, 0);
        assert_eq!(validate_image_bytes(&huge), None);
    }

    #[test]
    fn suggestion_letter_falls_back_for_non_alpha_titles() {
        assert_eq!(suggestion_letter("Arrival"), 'a');
        assert_eq!(suggestion_letter("1979"), 'a');
        assert_eq!(suggestion_letter("Étoile"), 'a');
        assert_eq!(suggestion_letter("  Alien"), 'a');
    }
}
