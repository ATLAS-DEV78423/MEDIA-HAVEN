//! Request authentication, per ADR-0005 §2.
//!
//! Every protected request carries:
//!
//! ```text
//! Authorization: Bearer <token>
//! X-MediaHaven-Timestamp: <unix seconds>
//! X-MediaHaven-Sig: <lowercase hex HMAC-SHA256>
//! ```
//!
//! where the HMAC key is the token and the message is the canonical string
//!
//! ```text
//! <timestamp> "\n" <METHOD> "\n" <path> "\n" <hex sha256 of the raw body>
//! ```
//!
//! The signature binds the secret to the method, path, body and time. A token
//! recovered from a log is therefore not by itself enough to replay a captured
//! request, and a signature reused against a different route or body fails.
//!
//! The timestamp must be within ±30 s of the server clock, and every comparison
//! is constant time.

use std::time::{SystemTime, UNIX_EPOCH};

use app_core::domain::{AppError, AppErrorCode};
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::routes::ApiError;
use crate::state::AppState;

pub const HEADER_TIMESTAMP: &str = "x-mediahaven-timestamp";
pub const HEADER_SIGNATURE: &str = "x-mediahaven-sig";

/// Accepted clock skew for the request timestamp (ADR-0005 §2).
pub const MAX_SKEW_SECONDS: i64 = 30;

/// Body ceiling for every route (ADR-0005 §4).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

type HmacSha256 = Hmac<Sha256>;

/// Lowercase hex SHA-256 of the raw request body.
pub fn body_digest(body: &[u8]) -> String {
    hex::encode(Sha256::digest(body))
}

/// The exact string that is signed for a request.
pub fn canonical_message(timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    format!(
        "{timestamp}\n{method}\n{path}\n{}",
        body_digest(body)
    )
}

/// Lowercase hex HMAC-SHA256 of the canonical message, keyed with the token.
pub fn sign(token: &str, timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    // HMAC accepts keys of any length, so this cannot fail.
    let mut mac = HmacSha256::new_from_slice(token.as_bytes())
        .expect("HMAC-SHA256 accepts any key length");
    mac.update(canonical_message(timestamp, method, path, body).as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn unauthorized() -> AppError {
    AppError::new(
        AppErrorCode::CredentialMismatch,
        "Request credentials were rejected.",
        false,
    )
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn unix_now() -> Result<i64, AppError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            AppError::new(
                AppErrorCode::Internal,
                "System clock is before the Unix epoch.",
                true,
            )
        })?
        .as_secs();
    i64::try_from(seconds).map_err(|_| {
        AppError::new(AppErrorCode::Internal, "System clock is out of range.", true)
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.ct_eq(right).into()
}

/// Authenticate the request, then hand the untouched body to the handler.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();

    let result = authorize(
        &state.token,
        &parts.headers,
        parts.method.as_str(),
        parts.uri.path(),
        body,
    )
    .await;

    match result {
        Ok(body) => next.run(Request::from_parts(parts, body)).await,
        Err(error) => ApiError(error).into_response(),
    }
}

async fn authorize(
    token: &str,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: Body,
) -> Result<Body, AppError> {
    let presented = bearer(headers).ok_or_else(unauthorized)?;
    if !constant_time_eq(token.as_bytes(), presented.as_bytes()) {
        return Err(unauthorized());
    }

    let timestamp = headers
        .get(HEADER_TIMESTAMP)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(unauthorized)?;
    let sent_at: i64 = timestamp.parse().map_err(|_| unauthorized())?;
    let now = unix_now()?;
    if (now - sent_at).abs() > MAX_SKEW_SECONDS {
        return Err(unauthorized());
    }

    let signature = headers
        .get(HEADER_SIGNATURE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(unauthorized)?;

    let bytes = to_bytes(body, MAX_BODY_BYTES).await.map_err(|_| {
        AppError::new(
            AppErrorCode::InvalidRequest,
            format!("Request body exceeds {MAX_BODY_BYTES} bytes."),
            false,
        )
    })?;

    let expected = sign(token, timestamp, method, path, &bytes);
    if !constant_time_eq(expected.as_bytes(), signature.trim().to_ascii_lowercase().as_bytes()) {
        return Err(unauthorized());
    }

    Ok(Body::from(bytes))
}
