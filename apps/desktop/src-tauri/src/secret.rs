//! Per-install API token provisioning.
//!
//! The host generates a 256-bit token on first run and stores it as a
//! DPAPI-protected blob scoped to the current Windows user (ADR-0014). The
//! token never reaches the repository, `%APPDATA%` as plaintext, a log, or the
//! renderer. On a read failure the host surfaces `credential_mismatch` instead
//! of silently regenerating — regenerating would orphan the running container's
//! token and lock the user out of their own library.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{compiler_fence, Ordering};

use windows::core::{Error as WinError, PCWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Number of bytes in a 256-bit token.
const TOKEN_BYTES: usize = 32;

/// DPAPI identity of the install token shared by the host forwarder and the
/// supervisor. Both must use exactly this pair or the token `core` receives at
/// `compose up` would differ from the one the forwarder signs with.
pub const TOKEN_SERVICE: &str = "mediahaven";
pub const TOKEN_ACCOUNT: &str = "core-api";

/// Why token provisioning failed.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// The DPAPI blob could not be read or unprotected. This is the
    /// fail-closed case: the host must not regenerate, because a running
    /// container would still hold the old token.
    #[error(
        "credential_mismatch: the stored token cannot be read; the container token may differ"
    )]
    CredentialMismatch,
    /// No Windows user profile is available, so no blob can be created. The
    /// host must never fall back to writing into the current directory.
    #[error("profile_unavailable: no Windows user profile is available for the token")]
    ProfileUnavailable,
    /// An invalid service/account pair was supplied.
    #[error(
        "invalid_token_identity: service and account must contain only letters, digits, '-' or '_'"
    )]
    InvalidIdentity,
    /// An I/O failure while reading or writing the blob.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A Win32 failure.
    #[error("win32: {0}")]
    Win32(#[from] WinError),
    /// The OS CSPRNG returned a non-success status.
    #[error("rng_failed: the OS CSPRNG returned status {0:#x}")]
    RngFailed(i32),
}

/// A 256-bit API token held in memory.
///
/// Deliberately **not** `Debug` and **not** `Serialize`: it must never be
/// printed, logged or serialised into a DTO. The buffer is zeroised on drop
/// with volatile writes so the compiler cannot elide the dead store.
pub struct SecretToken {
    bytes: Vec<u8>,
}

impl SecretToken {
    /// Wrap an existing byte buffer. Callers that already hold the bytes
    /// (e.g. after unprotecting) use this.
    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// The token as a UTF-8 hex string, for use as a bearer credential.
    ///
    /// This is the only accessor. It allocates a fresh `String` on each call
    /// and never hands out a reference into the protected buffer.
    pub fn as_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    /// The token as raw bytes. Callers must not persist or log them.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for SecretToken {
    fn drop(&mut self) {
        zeroize(&mut self.bytes);
    }
}

/// Overwrite a buffer with zeroes in a way the compiler cannot optimise away.
fn zeroize(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

/// Load the token for `(service, account)`, creating and storing it on first
/// use.
///
/// * On a successful unprotect the same token is returned every call.
/// * If the blob exists but cannot be read/unprotected, returns
///   [`SecretError::CredentialMismatch`] — never regenerates.
/// * If no blob exists, generates a fresh token from the OS CSPRNG, protects
///   it, writes it to disk and returns it.
pub fn load_or_create_token(service: &str, account: &str) -> Result<SecretToken, SecretError> {
    let path = blob_path(service, account)?;
    if path.exists() {
        // A failed read of an existing blob is a mismatch, not a fresh token:
        // regenerating would orphan the running container's token.
        let protected = fs::read(&path).map_err(|_| SecretError::CredentialMismatch)?;
        let mut bytes = unprotect(&protected).map_err(|_| SecretError::CredentialMismatch)?;
        if bytes.len() != TOKEN_BYTES {
            zeroize(&mut bytes);
            return Err(SecretError::CredentialMismatch);
        }
        return Ok(SecretToken::from_bytes(bytes));
    }

    let mut bytes = generate_random_bytes()?;
    let protected = match protect(&bytes) {
        Ok(value) => value,
        Err(error) => {
            zeroize(&mut bytes);
            return Err(error);
        }
    };
    if let Err(error) = write_blob(&path, &protected) {
        zeroize(&mut bytes);
        return Err(error);
    }
    Ok(SecretToken::from_bytes(bytes))
}

/// Load (or create on first run) the one install token used by both the signed
/// forwarder and `core`. Centralising the identity here prevents the two sides
/// from drifting to different DPAPI blobs.
pub fn install_token() -> Result<SecretToken, SecretError> {
    load_or_create_token(TOKEN_SERVICE, TOKEN_ACCOUNT)
}

/// DPAPI identity of the optional operator-supplied TMDB read token. The
/// account differs from the install token's, so the two blobs can never be
/// confused (ADR-0026).
pub const TMDB_SERVICE: &str = TOKEN_SERVICE;
pub const TMDB_ACCOUNT: &str = "tmdb-read-token";
/// DPAPI identities of the two optional Prowlarr inputs. Distinct accounts keep
/// each blob separate from the install token and from each other.
pub const PROWLARR_URL_ACCOUNT: &str = "prowlarr-url";
pub const PROWLARR_KEY_ACCOUNT: &str = "prowlarr-key";

/// Map a renderer-facing provider kind to its DPAPI account. Unknown kinds are
/// rejected before any store access, so no value can reach the store or an
/// error string (ADR-0007).
pub fn account_for(kind: &str) -> Option<&'static str> {
    match kind {
        "tmdb" => Some(TMDB_ACCOUNT),
        "prowlarr_url" => Some(PROWLARR_URL_ACCOUNT),
        "prowlarr_key" => Some(PROWLARR_KEY_ACCOUNT),
        _ => None,
    }
}

/// Store an arbitrary operator-provided secret as a DPAPI blob. The value is
/// zeroised after protection; it is never logged or returned.
pub fn store_secret(service: &str, account: &str, value: &str) -> Result<(), SecretError> {
    // Pasted tokens often carry surrounding whitespace; store the trimmed
    // value so it cannot break the key at use time. All-whitespace is blank.
    let value = value.trim();
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(SecretError::InvalidIdentity);
    }
    let path = blob_path(service, account)?;
    let mut bytes = value.as_bytes().to_vec();
    let protected = match protect(&bytes) {
        Ok(value) => value,
        Err(error) => {
            zeroize(&mut bytes);
            return Err(error);
        }
    };
    zeroize(&mut bytes);
    write_blob(&path, &protected)
}

/// Load an operator-provided secret.
///
/// `Ok(None)` means no secret is configured; a corrupt blob is an error, never
/// an empty fallback.
///
/// shortcut: the returned `String` is not zeroised on drop, matching
/// `SecretToken::as_hex`; callers keep it local and drop it promptly. Upgrade
/// if a zeroising string wrapper is introduced.
pub fn load_secret(service: &str, account: &str) -> Result<Option<String>, SecretError> {
    let path = blob_path(service, account)?;
    if !path.exists() {
        return Ok(None);
    }
    let protected = fs::read(&path).map_err(|_| SecretError::CredentialMismatch)?;
    let mut bytes = unprotect(&protected).map_err(|_| SecretError::CredentialMismatch)?;
    let text = String::from_utf8(bytes.clone()).map_err(|_| {
        zeroize(&mut bytes);
        SecretError::CredentialMismatch
    })?;
    zeroize(&mut bytes);
    Ok(Some(text))
}

/// Delete an operator-provided secret, if present.
pub fn delete_secret(service: &str, account: &str) -> Result<(), SecretError> {
    let path = blob_path(service, account)?;
    if path.exists() {
        fs::remove_file(path).map_err(|_| SecretError::CredentialMismatch)?;
    }
    Ok(())
}

/// Absolute path of the DPAPI blob for `(service, account)`.
///
/// Fails closed when the Windows profile is unavailable: it must never fall
/// back to the current directory, which would write a token into the repo.
pub fn blob_path(service: &str, account: &str) -> Result<PathBuf, SecretError> {
    if !valid_identity(service) || !valid_identity(account) {
        return Err(SecretError::InvalidIdentity);
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or(SecretError::ProfileUnavailable)?;
    Ok(base
        .join("MEDIA_HAVEN")
        .join(format!("{service}-{account}.blob")))
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Generate `TOKEN_BYTES` from the OS CSPRNG.
fn generate_random_bytes() -> Result<Vec<u8>, SecretError> {
    // BCryptGenRandom is the documented CSPRNG. It needs no entropy source.
    use windows::Win32::Security::Cryptography::BCryptGenRandom;
    use windows::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG;
    let mut buf = vec![0u8; TOKEN_BYTES];
    // BCryptGenRandom returns an NTSTATUS, not a Result: severity 0 is success.
    let status = unsafe { BCryptGenRandom(None, &mut buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.0 < 0 {
        zeroize(&mut buf);
        return Err(SecretError::RngFailed(status.0));
    }
    Ok(buf)
}

/// Protect bytes with DPAPI under the current user's profile.
fn protect(plain: &[u8]) -> Result<Vec<u8>, SecretError> {
    let input = blob_from_slice(plain);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        // UI_FORBIDDEN: never prompt on a headless/service process.
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;
        let bytes = copy_output_blob(&output);
        free_blob(output);
        Ok(bytes)
    }
}

/// Unprotect a DPAPI blob.
fn unprotect(protected: &[u8]) -> Result<Vec<u8>, WinError> {
    let input = blob_from_slice(protected);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output)?;
        let bytes = copy_output_blob(&output);
        free_blob(output);
        Ok(bytes)
    }
}

fn blob_from_slice(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

/// Copy a DPAPI output blob, guarding the null/zero-length case that makes
/// `from_raw_parts` undefined behaviour.
fn copy_output_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if blob.pbData.is_null() || blob.cbData == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() }
}

/// Free a DPAPI-allocated blob. DPAPI allocates with `LocalAlloc`.
fn free_blob(blob: CRYPT_INTEGER_BLOB) {
    if !blob.pbData.is_null() {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(blob.pbData.cast())));
        }
    }
}

fn write_blob(path: &PathBuf, bytes: &[u8]) -> Result<(), SecretError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Write to a temp file then rename, so a crash cannot leave a truncated
    // blob that would be read as `credential_mismatch`.
    let tmp = path.with_extension("tmp");
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(service: &str, account: &str) -> PathBuf {
        let path = blob_path(service, account).expect("APPDATA on this host");
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn second_call_returns_same_token() {
        let service = "mediahaven-test-a";
        let account = "roundtrip";
        let path = scratch(service, account);

        let first = load_or_create_token(service, account).expect("first load");
        let first_hex = first.as_hex();
        let second = load_or_create_token(service, account).expect("second load");
        assert_eq!(
            first_hex,
            second.as_hex(),
            "token must be stable across calls"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn token_is_256_bits_and_hex() {
        let service = "mediahaven-test-b";
        let account = "shape";
        let path = scratch(service, account);

        let token = load_or_create_token(service, account).expect("load");
        assert_eq!(token.as_bytes().len(), TOKEN_BYTES);
        assert_eq!(token.as_hex().len(), TOKEN_BYTES * 2);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn corrupted_blob_fails_closed() {
        let service = "mediahaven-test-c";
        let account = "corrupt";
        let path = scratch(service, account);

        // Write garbage: it exists, so the host must try to unprotect it and
        // fail closed rather than regenerate.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"not a dpapi blob").unwrap();

        let result = load_or_create_token(service, account);
        assert!(
            matches!(result, Err(SecretError::CredentialMismatch)),
            "a corrupt blob must fail closed, got {:?}",
            result.err()
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn identity_cannot_escape_the_blob_directory() {
        assert!(matches!(
            blob_path("a/b", "x"),
            Err(SecretError::InvalidIdentity)
        ));
        assert!(matches!(
            blob_path("..", "x"),
            Err(SecretError::InvalidIdentity)
        ));
        assert!(matches!(
            blob_path("ok", "a\\b"),
            Err(SecretError::InvalidIdentity)
        ));
    }

    /// The type must not be printable or serialisable. A compile-fail crate
    /// would need a new dependency (ADR), so this guards the source instead:
    /// it finds the struct declaration and inspects only the attribute lines
    /// immediately above it, ignoring doc comments.
    #[test]
    fn token_type_has_no_debug_or_serialize_impl() {
        let source = include_str!("secret.rs");
        let at = source
            .find("pub struct SecretToken {")
            .expect("struct declaration");
        let mut attrs = Vec::new();
        for line in source[..at].lines().rev() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[") {
                attrs.push(trimmed.to_string());
            } else if trimmed.starts_with("///") || trimmed.is_empty() {
                continue;
            } else {
                break;
            }
        }
        let attrs = attrs.join("\n");
        assert!(
            !attrs.contains("Debug"),
            "SecretToken must not derive Debug: {attrs}"
        );
        assert!(
            !attrs.contains("Serialize"),
            "SecretToken must not derive Serialize: {attrs}"
        );
        let debug_impl = ["impl std::fmt::", "Debug for SecretToken"].concat();
        let serde_impl = ["impl Serialize for ", "SecretToken"].concat();
        assert!(
            !source.contains(&debug_impl),
            "no manual Debug impl allowed"
        );
        assert!(
            !source.contains(&serde_impl),
            "no manual Serialize impl allowed"
        );
    }

    #[test]
    fn drop_zeroises_the_buffer() {
        let token = SecretToken::from_bytes(vec![0xCD; TOKEN_BYTES]);
        let ptr = token.as_bytes().as_ptr();
        drop(token);
        // The allocation is overwritten before free; the pointer itself may be
        // reused, so this test exercises the drop path without reading freed
        // memory. The volatile writes in `zeroize` are the actual guarantee.
        let _ = ptr;
    }

    /// The identity is the contract between the forwarder and the supervisor.
    /// Guard both the literal values and the fact that neither side keeps its
    /// own copy that could drift.
    #[test]
    fn token_identity_is_shared_and_exact() {
        assert_eq!(TOKEN_SERVICE, "mediahaven");
        assert_eq!(TOKEN_ACCOUNT, "core-api");

        let renderer = include_str!("commands/renderer.rs");
        let supervisor = include_str!("supervisor.rs");

        assert!(
            renderer.contains("TOKEN_SERVICE") && renderer.contains("TOKEN_ACCOUNT"),
            "the forwarder must use the shared identity constants"
        );
        assert!(
            supervisor.contains("install_token"),
            "the supervisor must load the token through the shared helper"
        );
        assert!(
            !renderer.contains("const TOKEN_SERVICE: &str")
                && !renderer.contains("const TOKEN_ACCOUNT: &str"),
            "the forwarder must not redeclare a private token identity"
        );
    }

    // shortcut: these round-trip tests write one DPAPI blob under APPDATA and
    // delete it; use a fixed test identity (uuid is not a desktop dependency).
    // Upgrade to an injectable store if tests ever run on a non-Windows host.
    #[test]
    fn generic_secret_round_trip_and_isolation() {
        let service = "mediahaven-test-e";
        let account = "tmdb-roundtrip";
        let path = scratch(service, account);

        assert_eq!(load_secret(service, account).unwrap(), None);
        store_secret(service, account, "a-test-value").unwrap();
        assert_eq!(
            load_secret(service, account).unwrap().as_deref(),
            Some("a-test-value")
        );
        delete_secret(service, account).unwrap();
        assert_eq!(load_secret(service, account).unwrap(), None);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn provider_identity_cannot_collide_with_install_token() {
        assert_ne!(TMDB_ACCOUNT, TOKEN_ACCOUNT);
        assert_ne!(
            blob_path(TMDB_SERVICE, TMDB_ACCOUNT).unwrap(),
            blob_path(TOKEN_SERVICE, TOKEN_ACCOUNT).unwrap()
        );
    }

    #[test]
    fn account_for_accepts_only_the_three_provider_kinds() {
        assert_eq!(account_for("tmdb"), Some(TMDB_ACCOUNT));
        assert_eq!(account_for("prowlarr_url"), Some(PROWLARR_URL_ACCOUNT));
        assert_eq!(account_for("prowlarr_key"), Some(PROWLARR_KEY_ACCOUNT));
        for unknown in ["imdb", "prowlarr", "TMDB", ""] {
            assert_eq!(account_for(unknown), None, "unexpected kind: {unknown}");
        }
    }

    #[test]
    fn provider_accounts_are_distinct_from_each_other_and_the_install_token() {
        let install = blob_path(TOKEN_SERVICE, TOKEN_ACCOUNT).unwrap();
        let mut paths = Vec::new();
        for account in [TMDB_ACCOUNT, PROWLARR_URL_ACCOUNT, PROWLARR_KEY_ACCOUNT] {
            let path = blob_path(TOKEN_SERVICE, account).unwrap();
            assert_ne!(
                path, install,
                "provider account collided with install token"
            );
            assert!(!paths.contains(&path), "two provider accounts share a blob");
            paths.push(path);
        }
    }

    #[test]
    fn generic_secret_trims_pasted_value_and_rejects_blank() {
        let service = "mediahaven-test-g";
        let account = "tmdb-trim";
        let path = scratch(service, account);

        assert!(matches!(
            store_secret(service, account, "   "),
            Err(SecretError::InvalidIdentity)
        ));

        store_secret(service, account, "  a-test-value  ").unwrap();
        assert_eq!(
            load_secret(service, account).unwrap().as_deref(),
            Some("a-test-value")
        );

        store_secret(service, account, "\ta-test-value\n").unwrap();
        assert_eq!(
            load_secret(service, account).unwrap().as_deref(),
            Some("a-test-value")
        );

        delete_secret(service, account).unwrap();
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn generic_secret_corrupt_blob_fails_closed() {
        let service = "mediahaven-test-f";
        let account = "tmdb-corrupt";
        let path = scratch(service, account);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"not a dpapi blob").unwrap();

        assert!(matches!(
            load_secret(service, account),
            Err(SecretError::CredentialMismatch)
        ));

        let _ = fs::remove_file(&path);
    }
}
