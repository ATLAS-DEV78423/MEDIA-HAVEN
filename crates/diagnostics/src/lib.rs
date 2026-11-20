//! Builds a local-only diagnostics bundle. All redaction is pure string work:
//! the crate performs no file, network, or system I/O of its own.

use app_core::errors::AppError;

/// Raw, untrusted inputs. `create_bundle` redacts each field before emitting.
pub struct BundleInput {
    pub log_lines: Vec<String>,
    pub config_shape: String,
    pub schema_version: String,
    pub health_snapshot: String,
}

/// Redaction policy. Filenames are hidden unless explicitly opted in.
#[derive(Default)]
pub struct BundleOptions {
    pub include_filenames: bool,
}

/// Produce the bundle text. The caller chooses where to write it; this crate
/// never touches the filesystem or the network.
pub fn create_bundle(input: BundleInput, opts: BundleOptions) -> Result<String, AppError> {
    let logs = input
        .log_lines
        .iter()
        .map(|line| redact(line, opts.include_filenames))
        .collect::<Vec<_>>()
        .join("\n");

    let schema = redact(&input.schema_version, opts.include_filenames);
    let health = redact(&input.health_snapshot, opts.include_filenames);
    let config = redact(&input.config_shape, opts.include_filenames);

    Ok(format!(
        "MEDIA_HAVEN diagnostics bundle (redacted, local-only)\n\
         schema_version: {schema}\n\n\
         health_snapshot:\n{health}\n\n\
         config_shape:\n{config}\n\n\
         logs:\n{logs}\n"
    ))
}

fn redact(input: &str, include_filenames: bool) -> String {
    let without_home = redact_home_paths(input);
    let without_files = if include_filenames {
        without_home
    } else {
        redact_filenames(&without_home)
    };
    redact_tokens(&without_files)
}

fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// Collapse runs of 32 or more hexadecimal characters into a marker.
fn redact_tokens(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if is_hex(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_hex(bytes[i]) {
                i += 1;
            }
            if i - start >= 32 {
                out.extend_from_slice(b"[REDACTED_TOKEN]");
            } else {
                out.extend_from_slice(&bytes[start..i]);
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("ASCII markers preserve UTF-8")
}

/// Return the index just past a `<drive>:\Users\<name>` prefix seen at `i`.
fn home_path_end(b: &[u8], i: usize) -> Option<usize> {
    if i + 9 > b.len()
        || !b[i].is_ascii_alphabetic()
        || b[i + 1] != b':'
        || !matches!(b[i + 2], b'\\' | b'/')
        || !b[i + 3..i + 8].eq_ignore_ascii_case(b"users")
        || !matches!(b[i + 8], b'\\' | b'/')
    {
        return None;
    }
    let mut end = i + 9;
    while end < b.len() && !matches!(b[end], b'\\' | b'/') {
        end += 1;
    }
    (end > i + 9).then_some(end)
}

/// Replace a Windows home-path prefix with `[HOME]`, keeping the path separator.
fn redact_home_paths(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = home_path_end(bytes, i) {
            out.extend_from_slice(b"[HOME]");
            if end < bytes.len() && matches!(bytes[end], b'\\' | b'/') {
                out.push(bytes[end]);
                i = end + 1;
            } else {
                i = end;
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("ASCII markers preserve UTF-8")
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Return `(start, end)` of a `stem.ext` filename beginning exactly at `i`.
fn filename_at(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut p = i;
    while p < b.len() && is_name_byte(b[p]) {
        p += 1;
    }
    if p == i || p >= b.len() || b[p] != b'.' {
        return None;
    }
    let ext_start = p + 1;
    let mut end = ext_start;
    while end < b.len() && b[end].is_ascii_alphanumeric() && end - ext_start < 8 {
        end += 1;
    }
    let ext_len = end - ext_start;
    if ext_len == 0 {
        return None;
    }
    Some((i, end))
}

/// Replace `stem.ext` filename-shaped tokens with a marker.
fn redact_filenames(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some((_, end)) = filename_at(bytes, i) {
            out.extend_from_slice(b"[REDACTED_FILENAME]");
            i = end;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("ASCII markers preserve UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(token: &str, path: &str) -> BundleInput {
        BundleInput {
            log_lines: vec![format!("auth token={token} opened {path}")],
            config_shape: "mode=local".to_string(),
            schema_version: "12".to_string(),
            health_snapshot: "ok".to_string(),
        }
    }

    #[test]
    fn default_options_redact_token_home_and_filename() {
        let token = "a".repeat(64);
        let path = r"C:\Users\Alice\Documents\secret.mkv";
        let out = create_bundle(sample(&token, path), BundleOptions::default()).unwrap();

        assert!(!out.contains(&token), "64-hex token leaked");
        assert!(!out.contains(r"C:\Users\Alice"), "home path leaked");
        assert!(!out.contains(path), "full secret path leaked");
        assert!(
            !out.contains("secret.mkv"),
            "filename leaked when not opted in"
        );
    }

    #[test]
    fn include_filenames_keeps_filename_but_still_redacts_token_and_home() {
        let token = "0123456789abcdef".repeat(4);
        let path = r"C:\Users\Alice\Documents\secret.mkv";
        let out = create_bundle(
            sample(&token, path),
            BundleOptions {
                include_filenames: true,
            },
        )
        .unwrap();

        assert!(!out.contains(&token), "64-hex token leaked");
        assert!(!out.contains(r"C:\Users\Alice"), "home path leaked");
        assert!(
            out.contains("secret.mkv"),
            "opted-in filename should remain"
        );
    }

    #[test]
    fn token_redaction_boundary_is_32_hex_chars() {
        let short = "b".repeat(31);
        let long = "c".repeat(32);
        let out = create_bundle(
            BundleInput {
                log_lines: vec![format!("short={short} long={long}")],
                config_shape: String::new(),
                schema_version: String::new(),
                health_snapshot: String::new(),
            },
            BundleOptions::default(),
        )
        .unwrap();

        assert!(out.contains(&short), "31-hex value must be preserved");
        assert!(!out.contains(&long), "32-hex token must be redacted");
    }
}
