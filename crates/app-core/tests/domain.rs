use app_core::domain::*;
use serde_json::json;

#[test]
fn app_error_code_serializes_to_frontend_strings() {
    let cases = [
        (AppErrorCode::RuntimeUnavailable, "runtime_unavailable"),
        (AppErrorCode::RuntimeUnhealthy, "runtime_unhealthy"),
        (AppErrorCode::BackendUnreachable, "backend_unreachable"),
        (AppErrorCode::CredentialMismatch, "credential_mismatch"),
        (AppErrorCode::LibraryRootOffline, "library_root_offline"),
        (
            AppErrorCode::LibraryRootUnreadable,
            "library_root_unreadable",
        ),
        (AppErrorCode::MediaUnsupported, "media_unsupported"),
        (AppErrorCode::MediaMissing, "media_missing"),
        (AppErrorCode::StorageFull, "storage_full"),
        (AppErrorCode::NotFound, "not_found"),
        (AppErrorCode::InvalidRequest, "invalid_request"),
        (AppErrorCode::Internal, "internal"),
    ];
    for (code, expected) in cases {
        let v = serde_json::to_value(code).unwrap();
        assert_eq!(v, json!(expected));
    }
}

#[test]
fn media_availability_missing_file_carries_last_seen_at() {
    let av = MediaAvailability::MissingFile {
        last_seen_at: Some("2026-01-01T00:00:00Z".to_string()),
    };
    let v = serde_json::to_value(&av).unwrap();
    assert_eq!(v["state"], "missing-file");
    assert_eq!(v["lastSeenAt"], "2026-01-01T00:00:00Z");

    let av2 = MediaAvailability::MissingFile { last_seen_at: None };
    let v2 = serde_json::to_value(&av2).unwrap();
    assert_eq!(v2["state"], "missing-file");
    assert!(v2["lastSeenAt"].is_null());
}

#[test]
fn app_error_serializes_camel_case() {
    let e = AppError::new(AppErrorCode::NotFound, "nope", false);
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["code"], "not_found");
    assert_eq!(v["message"], "nope");
    assert_eq!(v["retryable"], false);
}
