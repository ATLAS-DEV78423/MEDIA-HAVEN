//! The auth matrix required by ADR-0005 §2.
//!
//! Every protected route must refuse a request that is missing the bearer
//! token, carries the wrong token, carries a signature over a different body,
//! or whose timestamp falls outside the ±30 s replay window. `/healthz` stays
//! unauthenticated because the supervisor polls it before a token exists.

use std::time::{SystemTime, UNIX_EPOCH};

use app_core::application::LibraryService;
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};
use mediahaven_core::auth::{sign, HEADER_SIGNATURE, HEADER_TIMESTAMP};
use mediahaven_core::server::build_router;
use mediahaven_core::state::AppState;
use tower::ServiceExt;

const TOKEN: &str = "test-token-0123456789abcdef0123456789abcdef";
const BODY_LIMIT: usize = 1024 * 1024;

struct Fixture {
    router: Router,
    _dir: std::path::PathBuf,
}

async fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!("mediahaven-core-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let pool = connect_path(&dir.join("library.db"))
        .await
        .expect("connect");
    let state = AppState::new(LibraryService::new(SqliteLibraryRepository::new(pool)), TOKEN);
    Fixture {
        router: build_router(state),
        _dir: dir,
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs() as i64
}

/// Build a request signed exactly the way a client must sign it.
fn signed_request(
    method: &str,
    path: &str,
    body: &str,
    token: &str,
    timestamp_offset: i64,
) -> Request<Body> {
    let timestamp = (unix_now() + timestamp_offset).to_string();
    let signature = sign(token, &timestamp, method, path, body.as_bytes());
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(HEADER_TIMESTAMP, timestamp)
        .header(HEADER_SIGNATURE, signature)
        .body(Body::from(body.to_owned()))
        .expect("build request")
}

async fn status_of(router: Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = router.oneshot(request).await.expect("route");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), BODY_LIMIT)
        .await
        .expect("read body");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn healthz_is_unauthenticated() {
    let fixture = fixture().await;
    let request = Request::builder()
        .uri("/healthz")
        .body(Body::empty())
        .expect("build request");

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn readyz_requires_auth() {
    let fixture = fixture().await;
    let request = Request::builder()
        .uri("/readyz")
        .body(Body::empty())
        .expect("build request");

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["code"], "credential_mismatch");
}

#[tokio::test]
async fn readyz_accepts_a_signed_request() {
    let fixture = fixture().await;
    let request = signed_request("GET", "/readyz", "", TOKEN, 0);

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ready");
}

#[tokio::test]
async fn missing_token_is_rejected() {
    let fixture = fixture().await;
    let request = Request::builder()
        .method("POST")
        .uri("/library_home")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("build request");

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["code"], "credential_mismatch");
    assert_eq!(json["retryable"], false);
}

#[tokio::test]
async fn wrong_token_is_rejected() {
    let fixture = fixture().await;
    let request = signed_request(
        "POST",
        "/library_home",
        "{}",
        "not-the-right-token-0123456789abcdef",
        0,
    );

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["code"], "credential_mismatch");
}

#[tokio::test]
async fn tampered_body_is_rejected() {
    let fixture = fixture().await;
    let timestamp = unix_now().to_string();
    // Signature covers `{}` but the request carries a different payload.
    let signature = sign(TOKEN, &timestamp, "POST", "/library_home", b"{}");
    let request = Request::builder()
        .method("POST")
        .uri("/library_home")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(HEADER_TIMESTAMP, timestamp)
        .header(HEADER_SIGNATURE, signature)
        .body(Body::from(r#"{"injected":true}"#))
        .expect("build request");

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["code"], "credential_mismatch");
}

#[tokio::test]
async fn stale_timestamp_is_rejected() {
    let fixture = fixture().await;
    let request = signed_request("POST", "/library_home", "{}", TOKEN, -120);

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["code"], "credential_mismatch");
}

#[tokio::test]
async fn signature_for_another_route_is_rejected() {
    let fixture = fixture().await;
    let timestamp = unix_now().to_string();
    let signature = sign(TOKEN, &timestamp, "POST", "/library_watchlist", b"{}");
    let request = Request::builder()
        .method("POST")
        .uri("/library_home")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(HEADER_TIMESTAMP, timestamp)
        .header(HEADER_SIGNATURE, signature)
        .body(Body::from("{}"))
        .expect("build request");

    let (status, _) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signed_library_home_succeeds() {
    let fixture = fixture().await;
    let request = signed_request("POST", "/library_home", "{}", TOKEN, 0);

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("shelves").is_some(), "home screen shape");
}

#[tokio::test]
async fn unknown_fields_are_rejected() {
    let fixture = fixture().await;
    let body = r#"{"query":{"sort":"recent","limit":10,"offset":0,"nonsense":1}}"#;
    let request = signed_request("POST", "/library_movies", body, TOKEN, 0);

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "invalid_request");
}

#[tokio::test]
async fn empty_identifier_is_rejected() {
    let fixture = fixture().await;
    let body = r#"{"id":""}"#;
    let request = signed_request("POST", "/library_detail", body, TOKEN, 0);

    let (status, json) = status_of(fixture.router, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "invalid_request");
}
