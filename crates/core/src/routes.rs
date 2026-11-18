//! The named-operation HTTP surface.
//!
//! There is exactly one route per frontend command and no generic passthrough
//! (ADR-0005 §3). Every request body is an explicit, closed request type:
//! unknown fields are rejected rather than ignored, and the backend re-validates
//! every identifier even though the UI already did (ADR-0005 §4).

use app_core::domain::*;
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{async_trait, Json, Router};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::state::AppState;

/// Identifiers are opaque strings; anything longer than this is a bad request.
const MAX_ID_CHARS: usize = 200;
/// Free-text search is bounded so a body cannot be used as a query amplifier.
const MAX_SEARCH_CHARS: usize = 256;
/// One reorder may not rewrite an unbounded number of rows.
const MAX_REORDER_ITEMS: usize = 2000;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// An `AppError` rendered as JSON with the status code the UI expects.
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.code {
            AppErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            AppErrorCode::CredentialMismatch => StatusCode::UNAUTHORIZED,
            AppErrorCode::NotFound | AppErrorCode::MediaMissing => StatusCode::NOT_FOUND,
            AppErrorCode::MediaUnsupported => StatusCode::UNPROCESSABLE_ENTITY,
            AppErrorCode::StorageFull => StatusCode::INSUFFICIENT_STORAGE,
            AppErrorCode::RuntimeUnavailable
            | AppErrorCode::RuntimeUnhealthy
            | AppErrorCode::BackendUnreachable
            | AppErrorCode::LibraryRootOffline
            | AppErrorCode::LibraryRootUnreadable => StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self.0)).into_response()
    }
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError(AppError::new(AppErrorCode::InvalidRequest, message, false))
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// JSON extractor that normalises every parse failure into `invalid_request`.
pub struct ApiJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(invalid(rejection.body_text())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueryArgs {
    query: BrowseQuery,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchArgs {
    query: SearchQuery,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EpisodesArgs {
    series_id: String,
    season_number: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsArgs {
    patch: SettingsPatch,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaybackArgs {
    request: PlaybackStartRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointArgs {
    request: PlaybackCheckpointRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaIdArgs {
    media_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueueIdArgs {
    queue_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootAddArgs {
    display_name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootIdArgs {
    root_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OrderedIdsArgs {
    ordered_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn check_id(value: &str, field: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(invalid(format!("{field} must not be empty.")));
    }
    if value.chars().count() > MAX_ID_CHARS {
        return Err(invalid(format!("{field} is too long.")));
    }
    Ok(())
}

fn check_ordered_ids(ids: &[String]) -> Result<(), ApiError> {
    if ids.len() > MAX_REORDER_ITEMS {
        return Err(invalid("Too many items in one reorder."));
    }
    for id in ids {
        check_id(id, "orderedIds")?;
    }
    Ok(())
}

fn check_search(query: &SearchQuery) -> Result<(), ApiError> {
    if query.text.chars().count() > MAX_SEARCH_CHARS {
        return Err(invalid("Search text is too long."));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Routes that must answer before a token is available.
pub fn public_router() -> Router<AppState> {
    use axum::routing::get;

    Router::new().route("/healthz", get(healthz))
}

/// Every route that requires the install token and a valid signature.
pub fn protected_router() -> Router<AppState> {
    use axum::routing::{get, post};

    Router::new()
        .route("/readyz", get(readyz))
        .route("/library_home", post(library_home))
        .route("/library_movies", post(library_movies))
        .route("/library_shows", post(library_shows))
        .route("/library_detail", post(library_detail))
        .route("/library_episodes", post(library_episodes))
        .route("/library_search", post(library_search))
        .route("/library_roots", post(library_roots))
        .route("/library_root_add", post(library_root_add))
        .route("/library_root_remove", post(library_root_remove))
        .route("/library_scan", post(library_scan))
        .route("/library_watchlist", post(library_watchlist))
        .route("/queue_list", post(queue_list))
        .route("/downloads_list", post(downloads_list))
        .route("/settings_get", post(settings_get))
        .route("/settings_set", post(settings_set))
        .route("/playback_start", post(playback_start))
        .route("/playback_stop", post(playback_stop))
        .route("/playback_checkpoint", post(playback_checkpoint))
        .route("/watchlist_add", post(watchlist_add))
        .route("/watchlist_remove", post(watchlist_remove))
        .route("/watchlist_reorder", post(watchlist_reorder))
        .route("/queue_add", post(queue_add))
        .route("/queue_remove", post(queue_remove))
        .route("/queue_reorder", post(queue_reorder))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn healthz() -> Json<Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Ready means the database answers a real read, not merely that the process
/// is up (ADR-0005 §6). The UI waits for this before showing the library.
async fn readyz(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state.service.settings_get().await.map_err(|error| {
        ApiError(match error.code {
            AppErrorCode::Internal => AppError::new(
                AppErrorCode::RuntimeUnhealthy,
                "The library database is not answering.",
                true,
            ),
            _ => error,
        })
    })?;
    Ok(Json(serde_json::json!({ "status": "ready" })))
}

async fn library_home(State(state): State<AppState>) -> Result<Json<HomeScreen>, ApiError> {
    Ok(Json(state.service.home().await?))
}

async fn library_movies(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<QueryArgs>,
) -> Result<Json<Vec<MediaSummary>>, ApiError> {
    Ok(Json(state.service.movies(&args.query).await?))
}

async fn library_shows(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<QueryArgs>,
) -> Result<Json<Vec<SeriesDetail>>, ApiError> {
    Ok(Json(state.service.shows(&args.query).await?))
}

async fn library_detail(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<IdArgs>,
) -> Result<Json<MediaDetail>, ApiError> {
    check_id(&args.id, "id")?;
    Ok(Json(state.service.detail(&args.id).await?))
}

async fn library_episodes(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<EpisodesArgs>,
) -> Result<Json<Vec<EpisodeSummary>>, ApiError> {
    check_id(&args.series_id, "seriesId")?;
    if args.season_number < 0 {
        return Err(invalid("seasonNumber must not be negative."));
    }
    Ok(Json(
        state
            .service
            .episodes(&args.series_id, args.season_number)
            .await?,
    ))
}

async fn library_search(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<SearchArgs>,
) -> Result<Json<SearchResults>, ApiError> {
    check_search(&args.query)?;
    Ok(Json(state.service.search(&args.query).await?))
}

async fn library_roots(State(state): State<AppState>) -> Result<Json<LibraryScreen>, ApiError> {
    Ok(Json(state.service.roots().await?))
}

async fn library_root_add(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<RootAddArgs>,
) -> Result<Json<LibraryRoot>, ApiError> {
    if args.display_name.trim().is_empty() {
        return Err(invalid("displayName must not be empty."));
    }
    if args.path.trim().is_empty() {
        return Err(invalid("path must not be empty."));
    }
    // Containment/validity check happens in the scanner/policy layer; here we
    // at least require the path to be an existing directory.
    let p = std::path::Path::new(&args.path);
    if !p.is_dir() {
        return Err(ApiError(AppError::new(
            AppErrorCode::LibraryRootUnreadable,
            "The selected path is not a readable directory.",
            true,
        )));
    }
    Ok(Json(
        state
            .service
            .root_add(&args.display_name, &args.path)
            .await?,
    ))
}

async fn library_root_remove(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<RootIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.root_id, "rootId")?;
    state.service.root_remove(&args.root_id).await?;
    Ok(Json(Value::Null))
}

async fn library_scan(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<RootIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.root_id, "rootId")?;
    let roots = state.service.roots().await?;
    let root = roots
        .roots
        .into_iter()
        .find(|r| r.id == args.root_id)
        .ok_or_else(|| invalid("Unknown library root."))?;
    let path = root
        .path
        .clone()
        .ok_or_else(|| invalid("Library root has no path."))?;

    state
        .service
        .root_update(
            &args.root_id,
            root.item_count,
            app_core::domain::LibraryRootStatus::Scanning,
            root.last_scan_at.as_deref(),
            None,
        )
        .await?;

    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let scanner = media_library::Scanner::new();
    let outcome = scanner.scan(std::path::Path::new(&path), &cancel);

    let items: Vec<app_core::domain::MediaUpsert> = outcome
        .files
        .into_iter()
        .map(|f| {
            // A sidecar title wins over the filename-derived one; the filename
            // stays as the fallback for files with no sidecar.
            let title = f
                .metadata
                .as_ref()
                .and_then(|m| m.title.clone())
                .unwrap_or(f.title);
            app_core::domain::MediaUpsert {
                id: f.id,
                kind: f.kind,
                title,
                root_id: Some(args.root_id.clone()),
                path: Some(f.path),
                size_bytes: f.size_bytes,
                hash: f.hash,
                metadata: f.metadata,
            }
        })
        .collect();
    let scanned = items.len() as i64;

    // Same content under two paths is reported, never silently deduplicated.
    let mut by_hash: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in &items {
        if let (Some(hash), Some(path)) = (item.hash.as_deref(), item.path.as_deref()) {
            by_hash.entry(hash).or_default().push(path);
        }
    }
    let duplicates: Vec<Value> = by_hash
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(hash, paths)| serde_json::json!({ "hash": hash, "paths": paths }))
        .collect();
    let errors: Vec<String> = outcome
        .errors
        .iter()
        .map(|e| format!("{}: {}", e.path, e.message))
        .collect();

    let scan_result = state
        .service
        .media_replace_for_root(&args.root_id, &items)
        .await;

    let (status, err) = match scan_result {
        Ok(()) => (app_core::domain::LibraryRootStatus::Ok, None),
        Err(e) => (app_core::domain::LibraryRootStatus::Error, Some(e.message)),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let _ = state
        .service
        .root_update(&args.root_id, scanned, status, Some(&now), err.as_deref())
        .await;

    if let Some(e) = err {
        return Err(ApiError(AppError::new(AppErrorCode::Internal, e, true)));
    }
    Ok(Json(serde_json::json!({
        "scanned": scanned,
        "cancelled": outcome.cancelled,
        "errors": errors,
        "duplicates": duplicates,
    })))
}

async fn library_watchlist(
    State(state): State<AppState>,
) -> Result<Json<WatchlistScreen>, ApiError> {
    Ok(Json(state.service.watchlist().await?))
}

async fn queue_list(State(state): State<AppState>) -> Result<Json<QueueScreen>, ApiError> {
    Ok(Json(state.service.queue().await?))
}

async fn downloads_list(State(state): State<AppState>) -> Result<Json<DownloadsScreen>, ApiError> {
    Ok(Json(state.service.downloads().await?))
}

async fn settings_get(State(state): State<AppState>) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(state.service.settings_get().await?))
}

async fn settings_set(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<SettingsArgs>,
) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(state.service.settings_set(&args.patch).await?))
}

async fn playback_start(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<PlaybackArgs>,
) -> Result<Json<PlaybackSession>, ApiError> {
    check_id(&args.request.media_id, "request.mediaId")?;
    if let Some(position) = args.request.start_position_seconds {
        if !position.is_finite() || position < 0.0 {
            return Err(invalid(
                "request.startPositionSeconds must be a finite, non-negative number.",
            ));
        }
    }
    Ok(Json(state.service.playback_start(&args.request).await?))
}

async fn playback_stop(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state.service.playback_stop().await?;
    Ok(Json(Value::Null))
}

async fn playback_checkpoint(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<CheckpointArgs>,
) -> Result<Json<Value>, ApiError> {
    if !args.request.position_seconds.is_finite() || args.request.position_seconds < 0.0 {
        return Err(invalid(
            "request.positionSeconds must be a finite, non-negative number.",
        ));
    }
    if let Some(duration) = args.request.duration_seconds {
        if !duration.is_finite() || duration < 0.0 {
            return Err(invalid(
                "request.durationSeconds must be a finite, non-negative number.",
            ));
        }
    }
    state.service.playback_checkpoint(&args.request).await?;
    Ok(Json(Value::Null))
}

async fn watchlist_add(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<MediaIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.media_id, "mediaId")?;
    state.service.watchlist_add(&args.media_id).await?;
    Ok(Json(Value::Null))
}

async fn watchlist_remove(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<MediaIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.media_id, "mediaId")?;
    state.service.watchlist_remove(&args.media_id).await?;
    Ok(Json(Value::Null))
}

async fn watchlist_reorder(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<OrderedIdsArgs>,
) -> Result<Json<Value>, ApiError> {
    check_ordered_ids(&args.ordered_ids)?;
    state.service.watchlist_reorder(&args.ordered_ids).await?;
    Ok(Json(Value::Null))
}

async fn queue_add(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<MediaIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.media_id, "mediaId")?;
    state.service.queue_add(&args.media_id).await?;
    Ok(Json(Value::Null))
}

async fn queue_remove(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<QueueIdArgs>,
) -> Result<Json<Value>, ApiError> {
    check_id(&args.queue_id, "queueId")?;
    state.service.queue_remove(&args.queue_id).await?;
    Ok(Json(Value::Null))
}

async fn queue_reorder(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<OrderedIdsArgs>,
) -> Result<Json<Value>, ApiError> {
    check_ordered_ids(&args.ordered_ids)?;
    state.service.queue_reorder(&args.ordered_ids).await?;
    Ok(Json(Value::Null))
}
