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
        .route("/library_watchlist", post(library_watchlist))
        .route("/queue_list", post(queue_list))
        .route("/downloads_list", post(downloads_list))
        .route("/settings_get", post(settings_get))
        .route("/settings_set", post(settings_set))
        .route("/playback_start", post(playback_start))
        .route("/playback_stop", post(playback_stop))
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

async fn library_watchlist(
    State(state): State<AppState>,
) -> Result<Json<WatchlistScreen>, ApiError> {
    Ok(Json(state.service.watchlist().await?))
}

async fn queue_list(State(state): State<AppState>) -> Result<Json<QueueScreen>, ApiError> {
    Ok(Json(state.service.queue().await?))
}

async fn downloads_list(
    State(state): State<AppState>,
) -> Result<Json<DownloadsScreen>, ApiError> {
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
            return Err(invalid("request.startPositionSeconds must be a finite, non-negative number."));
        }
    }
    Ok(Json(state.service.playback_start(&args.request).await?))
}

async fn playback_stop(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state.service.playback_stop().await?;
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
