//! Bounded reader and parser for Kodi-style `.nfo` sidecars.
//!
//! Deliberately not a general XML parser: it reads the flat field elements an
//! NFO actually uses, and refuses anything that could make parsing expensive or
//! dangerous — a DTD or entity declaration, more than 256 KiB, control
//! characters, or non-UTF-8 bytes. It never fetches, follows a link, or writes.

use crate::model::{
    candidates, NfoDocument, NfoError, MAX_GENRES, MAX_GENRE_CHARS, MAX_NFO_BYTES,
    MAX_SYNOPSIS_CHARS, MAX_TITLE_CHARS,
};
use std::path::Path;

/// Look for a sidecar beside `media` and parse the first one found.
///
/// `Ok(None)` means there is no sidecar. `Err` means one exists but was
/// refused; the caller reports that rather than pretending the file is bare.
pub fn read_beside(media: &Path) -> Result<Option<NfoDocument>, NfoError> {
    for candidate in candidates(media) {
        let meta = match std::fs::symlink_metadata(&candidate) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(NfoError::Unreadable(err.to_string())),
        };
        // A symlink or directory named `*.nfo` is skipped, never followed.
        if !meta.is_file() {
            continue;
        }
        if meta.len() > MAX_NFO_BYTES {
            return Err(NfoError::TooLarge);
        }
        let bytes =
            std::fs::read(&candidate).map_err(|err| NfoError::Unreadable(err.to_string()))?;
        if bytes.len() as u64 > MAX_NFO_BYTES {
            return Err(NfoError::TooLarge);
        }
        return parse(&bytes).map(Some);
    }
    Ok(None)
}

pub fn parse(bytes: &[u8]) -> Result<NfoDocument, NfoError> {
    let text = std::str::from_utf8(bytes).map_err(|_| NfoError::NotUtf8)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let lowered = text.to_ascii_lowercase();
    if lowered.contains("<!doctype") || lowered.contains("<!entity") {
        return Err(NfoError::DoctypeRefused);
    }

    let synopsis = match first(&lowered, text, "plot", MAX_SYNOPSIS_CHARS)? {
        Some(value) => Some(value),
        None => first(&lowered, text, "outline", MAX_SYNOPSIS_CHARS)?,
    };

    let mut genres: Vec<String> = Vec::new();
    for raw in elements(text, "genre")? {
        if let Some(genre) = clean_text(&raw, MAX_GENRE_CHARS) {
            if !genres.iter().any(|g| g.eq_ignore_ascii_case(&genre)) {
                genres.push(genre);
            }
            if genres.len() == MAX_GENRES {
                break;
            }
        }
    }

    Ok(NfoDocument {
        title: first(&lowered, text, "title", MAX_TITLE_CHARS)?,
        year: first_number(&lowered, text, "year", 1870, 2200)?,
        synopsis,
        runtime_minutes: first_number(&lowered, text, "runtime", 1, 10_000)?,
        genres,
    })
}

/// Read one required element, returning `Ok(None)` when it is absent or blank.
/// `lowered` is `text` lowercased; indices line up because lowercasing ASCII
/// preserves byte length for the tag names we search for.
fn first(
    lowered: &str,
    text: &str,
    name: &str,
    max_chars: usize,
) -> Result<Option<String>, NfoError> {
    let closing = format!("</{name}>");
    let haystack = lowered;
    let mut from = 0usize;
    while let Some(rel) = haystack[from..].find('<') {
        let start = from + rel;
        let Some(close_rel) = haystack[start..].find('>') else {
            break;
        };
        let close = start + close_rel;
        let raw = &haystack[start + 1..close];
        let trimmed = raw.trim();
        let skipped =
            trimmed.starts_with('/') || trimmed.starts_with('!') || trimmed.starts_with('?');
        let self_closing = trimmed.ends_with('/');
        let tag = trimmed.split_whitespace().next().unwrap_or("");
        if !skipped && !self_closing && tag == name {
            let rest = &haystack[close + 1..];
            let Some(end) = rest.find(&closing) else {
                return Err(NfoError::Malformed(format!("missing {closing}")));
            };
            // Decode from the original text so casing survives.
            return decode(&text[close + 1..close + 1 + end])
                .map(|value| clean_text(&value, max_chars));
        }
        from = close + 1;
    }
    Ok(None)
}

fn first_number(
    lowered: &str,
    text: &str,
    name: &str,
    min: i64,
    max: i64,
) -> Result<Option<i64>, NfoError> {
    let Some(value) = first(lowered, text, name, 32)? else {
        return Ok(None);
    };
    // A value that is present but unusable is dropped for that field only; the
    // rest of the file is still worth having.
    Ok(value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|n| (min..=max).contains(n)))
}

/// All occurrences of an element, in document order.
fn elements(text: &str, name: &str) -> Result<Vec<String>, NfoError> {
    let lowered = text.to_ascii_lowercase();
    let closing = format!("</{name}>");
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = lowered[from..].find('<') {
        let start = from + rel;
        let Some(close_rel) = lowered[start..].find('>') else {
            break;
        };
        let close = start + close_rel;
        let trimmed = lowered[start + 1..close].trim();
        let skipped =
            trimmed.starts_with('/') || trimmed.starts_with('!') || trimmed.starts_with('?');
        let self_closing = trimmed.ends_with('/');
        let tag = trimmed.split_whitespace().next().unwrap_or("");
        if !skipped && !self_closing && tag == name {
            let rest = &lowered[close + 1..];
            let Some(end) = rest.find(&closing) else {
                return Err(NfoError::Malformed(format!("missing {closing}")));
            };
            out.push(decode(&text[close + 1..close + 1 + end])?);
            from = close + 1 + end + closing.len();
        } else {
            from = close + 1;
        }
    }
    Ok(out)
}

/// Decode the five XML entities NFO files use plus numeric references. Unknown
/// entities are refused rather than passed through, so nothing that looked like
/// markup reaches the database or the UI.
fn decode(raw: &str) -> Result<String, NfoError> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '<' => return Err(NfoError::Malformed("raw markup inside a value".to_string())),
            '&' => {
                let rest = &raw[index + 1..];
                let Some(end) = rest.find(';') else {
                    return Err(NfoError::Malformed("unterminated entity".to_string()));
                };
                let name = &rest[..end];
                if name.is_empty() || name.len() > 10 {
                    return Err(NfoError::Malformed("invalid entity".to_string()));
                }
                let decoded = match name {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => {
                        let Some(number) = name.strip_prefix('#') else {
                            return Err(NfoError::Malformed(format!("unknown entity &{name};")));
                        };
                        let code = if let Some(hex) = number
                            .strip_prefix('x')
                            .or_else(|| number.strip_prefix('X'))
                        {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            number.parse::<u32>().ok()
                        };
                        code.and_then(char::from_u32).ok_or_else(|| {
                            NfoError::Malformed(format!("invalid entity &{name};"))
                        })?
                    }
                };
                out.push(decoded);
                // Skip the entity name and the ';' in the source iterator.
                for _ in 0..=name.len() {
                    chars.next();
                }
            }
            ch if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' => {
                return Err(NfoError::ControlCharacters)
            }
            ch => out.push(ch),
        }
    }
    Ok(out)
}

/// Collapse runs of whitespace so a value fits one database column, and cap
/// its length. Blank values become `None`.
fn clean_text(raw: &str, max_chars: usize) -> Option<String> {
    let joined = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        return None;
    }
    let capped: String = joined.chars().take(max_chars).collect();
    if capped.trim().is_empty() {
        None
    } else {
        Some(capped)
    }
}
