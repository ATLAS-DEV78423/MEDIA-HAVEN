//! The named-operation HTTP surface.
//!
//! There is exactly one route per frontend command and no generic passthrough
//! (ADR-0005 §3). Every request body is an explicit, closed request type:
//! unknown fields are rejected rather than ignored, and the backend re-validates
//! every identifier even though the UI already did (ADR-0005 §4).

use app_core::domain::*;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{async_trait, Json, Router};
use media_metadata::{
    fetch_image_bytes, ArtworkCache, CatalogKind, ImdbArtworkProvider, TmdbProvider,
    DEFAULT_MAX_CACHE_BYTES,
};
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscoverArgs {
    kind: String,
    page: u32,
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
// Artwork (opt-in)
// ---------------------------------------------------------------------------

/// Serve a cached poster. `:key` is a 64-char SHA-256 hex digest, never a path,
/// so a request cannot traverse the artwork directory. The key is validated
/// here, before the join — `ArtworkCache` does not validate its own keys.
async fn artwork(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let valid = key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if !valid {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.artwork_dir.as_ref().join(&key);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let content_type = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else {
        "image/png"
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=604800"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response()
}

/// Cap on IMDb lookups plus CDN downloads in one enrichment batch. Each poster
/// costs a search (10 s timeout) and a fetch, done sequentially in one blocking
/// task, so an uncapped cold shelf could stall the response for tens of seconds.
const MAX_ENRICH_PER_REQUEST: usize = 12;

/// Fill one item's artwork with a relative `/artwork/<hash>` URL. Shared by
/// every DTO shape so lookup/cache/fetch logic lives in exactly one place.
fn fill_poster(
    tmdb: &TmdbProvider,
    imdb_artwork: &ImdbArtworkProvider,
    cache: &ArtworkCache,
    title: &str,
    year: Option<i64>,
    artwork: &mut Vec<ArtworkRef>,
) {
    // TMDB first. A usable cached or freshly downloaded poster decides the
    // single key we emit; every failure just falls through to IMDb.
    let mut key: Option<String> = None;
    if tmdb.options().enabled {
        if let Some(found) = tmdb.lookup_full(title, year) {
            if let Some(path) = found.poster_path {
                let poster_key = media_metadata::artwork_key(&path);
                if cache.get(&poster_key).is_some() {
                    key = Some(poster_key);
                } else if let Ok((bytes, kind)) = media_metadata::fetch_artwork_bytes(&path) {
                    if cache.put(&poster_key, &bytes, &kind).is_ok() {
                        key = Some(poster_key);
                    }
                }
            }
        }
    }

    if key.is_none() {
        let Some(url) = imdb_artwork.poster_url(title, year) else {
            return;
        };
        let imdb_key = media_metadata::artwork_key(&url);
        if cache.get(&imdb_key).is_none() {
            let Ok((bytes, kind)) = fetch_image_bytes(&url) else {
                return;
            };
            if cache.put(&imdb_key, &bytes, &kind).is_err() {
                return;
            }
        }
        key = Some(imdb_key);
    }

    if let Some(key) = key {
        artwork.push(ArtworkRef {
            variant: ArtworkVariant::Poster,
            url: Some(format!("/artwork/{key}")),
            alt: format!("{title} poster"),
        });
    }
}

/// One `spawn_blocking` for the whole batch: the TMDB/IMDb providers and image
/// cache are synchronous and may do disk I/O, but no tokio worker may block.
async fn enrich_artwork(state: &AppState, mut items: Vec<MediaSummary>) -> Vec<MediaSummary> {
    let fallback = items.clone();
    let tmdb = state.tmdb.clone();
    let imdb_artwork = state.imdb_artwork.clone();
    let artwork_dir = state.artwork_dir.clone();
    let filled = tokio::task::spawn_blocking(move || {
        let cache = ArtworkCache::new(artwork_dir.as_ref().clone(), DEFAULT_MAX_CACHE_BYTES);
        // shortcut: only the first MAX_ENRICH_PER_REQUEST items get a poster; the rest keep bundled art. Upgrade to a bounded-concurrency pool or a background refresh when shelves regularly exceed this cap or near-real-time freshness matters.
        for item in items.iter_mut().take(MAX_ENRICH_PER_REQUEST) {
            fill_poster(
                &tmdb,
                &imdb_artwork,
                &cache,
                &item.title,
                item.year,
                &mut item.artwork,
            );
        }
        items
    })
    .await;
    filled.unwrap_or(fallback)
}

/// Series detail has the same title/year/artwork shape but a different DTO.
async fn enrich_series_artwork(
    state: &AppState,
    mut items: Vec<SeriesDetail>,
) -> Vec<SeriesDetail> {
    let fallback = items.clone();
    let tmdb = state.tmdb.clone();
    let imdb_artwork = state.imdb_artwork.clone();
    let artwork_dir = state.artwork_dir.clone();
    let filled = tokio::task::spawn_blocking(move || {
        let cache = ArtworkCache::new(artwork_dir.as_ref().clone(), DEFAULT_MAX_CACHE_BYTES);
        // shortcut: only the first MAX_ENRICH_PER_REQUEST items get a poster; the rest keep bundled art. Upgrade to a bounded-concurrency pool or a background refresh when shelves regularly exceed this cap or near-real-time freshness matters.
        for item in items.iter_mut().take(MAX_ENRICH_PER_REQUEST) {
            fill_poster(
                &tmdb,
                &imdb_artwork,
                &cache,
                &item.title,
                item.year,
                &mut item.artwork,
            );
        }
        items
    })
    .await;
    filled.unwrap_or(fallback)
}

/// Movie and series detail are enriched in the same blocking batch.
async fn enrich_detail_artwork(state: &AppState, mut detail: MediaDetail) -> MediaDetail {
    let fallback = detail.clone();
    let tmdb = state.tmdb.clone();
    let imdb_artwork = state.imdb_artwork.clone();
    let artwork_dir = state.artwork_dir.clone();
    let filled = tokio::task::spawn_blocking(move || {
        let cache = ArtworkCache::new(artwork_dir.as_ref().clone(), DEFAULT_MAX_CACHE_BYTES);
        match &mut detail {
            MediaDetail::Movie(movie) => {
                fill_poster(
                    &tmdb,
                    &imdb_artwork,
                    &cache,
                    &movie.title,
                    movie.year,
                    &mut movie.artwork,
                );
            }
            MediaDetail::Series(series) => {
                fill_poster(
                    &tmdb,
                    &imdb_artwork,
                    &cache,
                    &series.title,
                    series.year,
                    &mut series.artwork,
                );
            }
        }
        detail
    })
    .await;
    filled.unwrap_or(fallback)
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Routes that must answer before a token is available.
pub fn public_router() -> Router<AppState> {
    use axum::routing::get;

    Router::new()
        .route("/healthz", get(healthz))
        // Public: cached bytes are already-public IMDb poster images and the
        // listener is loopback-only. GET keeps it out of the 24 POST commands.
        .route("/artwork/:key", get(artwork))
}

/// Every route that requires the install token and a valid signature.
pub fn protected_router() -> Router<AppState> {
    use axum::routing::{get, post};

    Router::new()
        .route("/readyz", get(readyz))
        // Host-internal source lookup (ADR-0015). GET, not POST: the renderer
        // command surface is POST-only and the parity test derives commands
        // from POST routes, so this stays OUT of the 24 renderer commands.
        // It still passes the signed auth middleware, like `/readyz`.
        .route("/playback_source/:media_id", get(playback_source))
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
        .route("/discover_catalog", post(discover_catalog))
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
    let mut screen = state.service.home().await?;
    screen.hero = match screen.hero {
        Some(mut hero) => {
            hero.media = enrich_artwork(&state, vec![hero.media]).await.remove(0);
            Some(hero)
        }
        None => None,
    };
    for shelf in &mut screen.shelves {
        shelf.items = enrich_artwork(&state, std::mem::take(&mut shelf.items)).await;
    }
    Ok(Json(screen))
}

async fn library_movies(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<QueryArgs>,
) -> Result<Json<Vec<MediaSummary>>, ApiError> {
    let items = state.service.movies(&args.query).await?;
    Ok(Json(enrich_artwork(&state, items).await))
}

async fn library_shows(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<QueryArgs>,
) -> Result<Json<Vec<SeriesDetail>>, ApiError> {
    let items = state.service.shows(&args.query).await?;
    Ok(Json(enrich_series_artwork(&state, items).await))
}

async fn library_detail(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<IdArgs>,
) -> Result<Json<MediaDetail>, ApiError> {
    check_id(&args.id, "id")?;
    let detail = state.service.detail(&args.id).await?;
    Ok(Json(enrich_detail_artwork(&state, detail).await))
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
    let mut results = state.service.search(&args.query).await?;
    results.items = enrich_artwork(&state, std::mem::take(&mut results.items)).await;
    Ok(Json(results))
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

/// Cap on poster downloads in one discovery request. Each poster costs a
/// sequential `fetch_artwork_bytes` (10 s timeout) inside the single blocking
/// task, so an uncapped cold page could stall the response for minutes.
const MAX_DISCOVER_POSTERS_PER_REQUEST: usize = 12;

/// Display-only TMDB catalogue. One blocking batch covers the provider fetch
/// and the poster cache; without a key nothing is fetched or read.
async fn discover_catalog(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<DiscoverArgs>,
) -> Result<Json<CatalogScreen>, ApiError> {
    let kind = match args.kind.as_str() {
        "movie" => CatalogKind::Movie,
        "tv" => CatalogKind::Tv,
        "anime" => CatalogKind::Anime,
        _ => return Err(invalid("unknown catalogue kind")),
    };
    let page = args.page.clamp(1, 20);
    if !state.discover.enabled() {
        return Ok(Json(CatalogScreen {
            available: false,
            entries: Vec::new(),
        }));
    }
    let discover = state.discover.clone();
    let artwork_dir = state.artwork_dir.clone();
    let entries = tokio::task::spawn_blocking(move || {
        let cache = ArtworkCache::new(artwork_dir.as_ref().clone(), DEFAULT_MAX_CACHE_BYTES);
        discover
            .discover(kind, page)
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let mut artwork = Vec::new();
                // shortcut: only the first MAX_DISCOVER_POSTERS_PER_REQUEST entries get a
                // downloaded poster (10 s each, sequential in one blocking task); the rest
                // stay display-only. Upgrade to a bounded-concurrency pool when shelves
                // regularly exceed this cap.
                if index < MAX_DISCOVER_POSTERS_PER_REQUEST {
                    if let Some(path) = entry.poster_path {
                        let key = media_metadata::artwork_key(&path);
                        if cache.get(&key).is_none() {
                            if let Ok((bytes, content_type)) =
                                media_metadata::fetch_artwork_bytes(&path)
                            {
                                let _ = cache.put(&key, &bytes, &content_type);
                            }
                        }
                        if cache.get(&key).is_some() {
                            artwork.push(ArtworkRef {
                                variant: ArtworkVariant::Poster,
                                url: Some(format!("/artwork/{key}")),
                                alt: format!("{} poster", entry.title),
                            });
                        }
                    }
                }
                CatalogEntryDto {
                    id: entry.id,
                    kind: match entry.kind {
                        CatalogKind::Movie => CatalogKindDto::Movie,
                        CatalogKind::Tv => CatalogKindDto::Tv,
                        CatalogKind::Anime => CatalogKindDto::Anime,
                    },
                    title: entry.title,
                    year: entry.year,
                    synopsis: entry.synopsis,
                    rating: entry.vote_average,
                    artwork,
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    Ok(Json(CatalogScreen {
        available: true,
        entries,
    }))
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
    let session = state.service.playback_start(&args.request).await?;
    // Task 6 fix: record the live playback session. This — not an inert
    // download_items.state field — is the "actively playing" cleanup gate.
    crate::acquisition::on_playback_started(&args.request.media_id);
    // Task 5: trigger the rolling buffer after the policy checks above. The
    // trigger swallows its own errors, so a prefetch failure can never fail the
    // playback session (plan risk A-6). Backend stays authoritative (ADR-0002).
    crate::acquisition::trigger_prefetch(&state, &args.request.media_id).await;
    Ok(Json(session))
}

async fn playback_stop(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state.service.playback_stop().await?;
    // Clear the authoritative live-session gate so cleanup can proceed later.
    crate::acquisition::on_playback_stopped();
    Ok(Json(Value::Null))
}

/// Host-internal (ADR-0015): the stored container-path identity for a media id.
/// It is authenticated like `/readyz` but is NOT a renderer command — the host
/// resolves this path through A9's mount map and never returns it to the WebView.
async fn playback_source(
    State(state): State<AppState>,
    Path(media_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_id(&media_id, "media_id")?;
    let path = state.service.media_source_path(&media_id).await?;
    Ok(Json(serde_json::json!({ "path": path })))
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
    // Task 6: evaluate cleanup only after a confirmed completion is recorded;
    // the trigger swallows its own errors and never deletes inline, so a
    // checkpoint can never fail playback (seeking/close never confirm).
    crate::acquisition::on_playback_checkpoint(&args.request.media_id).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::application::LibraryService;
    use axum::body::Body;
    use axum::http::Request;
    use media_metadata::tmdb::TmdbOptions;
    use media_metadata::{DiscoverOptions, DiscoverProvider};
    use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tower::ServiceExt;

    /// An allowlisted poster URL. The fake fetch returns only this string; the
    /// bytes are pre-seeded in the cache, so no network request occurs.
    const FAKE_URL: &str = "https://m.media-amazon.com/images/M/MV5Btest._V1_.jpg";

    async fn service_for_test() -> crate::state::LibraryServiceHandle {
        let dir = std::env::temp_dir().join(format!("mh-artwork-route-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        LibraryService::new(SqliteLibraryRepository::new(pool))
    }

    fn summary_for_test() -> MediaSummary {
        MediaSummary {
            id: "m1".into(),
            kind: MediaKind::Movie,
            title: "Arrival".into(),
            year: Some(2016),
            synopsis: None,
            runtime_minutes: None,
            genres: vec![],
            artwork: vec![],
            availability: MediaAvailability::Available,
            progress: None,
            imdb_rating: None,
            vote_count: None,
            age_rating: None,
        }
    }

    /// A non-hash key is refused before any filesystem join. A `../` URI is
    /// deliberately avoided: a client or router could normalise it away.
    #[tokio::test]
    async fn artwork_route_rejects_non_hash_keys() {
        let app =
            crate::server::build_router(AppState::new(service_for_test().await, "test-token"));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/artwork/not-a-hash")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// A failed lookup adds no artwork: the bundled art stays untouched.
    #[tokio::test]
    async fn imdb_artwork_failure_adds_no_artwork() {
        let provider = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Err("boom".into())));
        let state =
            AppState::new(service_for_test().await, "test-token").with_imdb_artwork(provider);
        let items = enrich_artwork(&state, vec![summary_for_test()]).await;
        assert!(items[0].artwork.is_empty());
    }

    /// Happy path with fakes only: a fake URL and a pre-seeded poster byte file,
    /// so no network call occurs. The DTO carries only a relative hashed URL.
    #[tokio::test]
    async fn imdb_artwork_adds_only_a_relative_hashed_url() {
        let dir = std::env::temp_dir().join(format!("mh-artwork-enabled-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");
        let key = media_metadata::artwork_key(FAKE_URL);
        std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");

        let provider = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Ok(FAKE_URL.into())));
        let state = AppState::new(service_for_test().await, "test-token")
            .with_imdb_artwork(provider)
            .with_artwork_dir(artwork_dir);

        let items = enrich_artwork(&state, vec![summary_for_test()]).await;
        let refs = &items[0].artwork;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].variant, ArtworkVariant::Poster);
        let expected = format!("/artwork/{key}");
        assert_eq!(refs[0].url.as_deref(), Some(expected.as_str()));
        let json = serde_json::to_string(&items[0]).unwrap();
        assert!(!json.contains("m.media-amazon.com"));
        assert!(!json.contains("v2.sg.media-imdb.com"));

        let response = crate::server::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(expected.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A cold 50-item shelf must not stall the response: enrichment is capped,
    /// but every input item is still returned (the rest keep bundled art).
    #[tokio::test]
    async fn enrichment_is_bounded_but_returns_every_item() {
        let dir = std::env::temp_dir().join(format!("mh-artwork-bound-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");
        let key = media_metadata::artwork_key(FAKE_URL);
        std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");

        let provider = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Ok(FAKE_URL.into())));
        let state = AppState::new(service_for_test().await, "test-token")
            .with_imdb_artwork(provider)
            .with_artwork_dir(artwork_dir);

        let items: Vec<MediaSummary> = (0..20)
            .map(|index| MediaSummary {
                id: format!("m{index}"),
                ..summary_for_test()
            })
            .collect();
        let enriched = enrich_artwork(&state, items).await;
        assert_eq!(enriched.len(), 20, "no item may be dropped or truncated");
        let with_art = enriched
            .iter()
            .filter(|item| !item.artwork.is_empty())
            .count();
        assert_eq!(with_art, MAX_ENRICH_PER_REQUEST);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A minimal TMDB search payload a fake fetch can return. Only title and
    /// poster_path are needed by `parse_result`; no network is involved.
    fn tmdb_json(poster_path: &str) -> String {
        format!(r#"{{"results":[{{"title":"Arrival","poster_path":"{poster_path}"}}]}}"#)
    }

    /// An enabled TMDB provider driven by a fake fetch, writing its metadata
    /// cache under `dir` so the test never touches the shared temp dir.
    fn fake_tmdb(dir: &std::path::Path, fetch: Box<media_metadata::tmdb::FetchFn>) -> TmdbProvider {
        TmdbProvider::with_fetch(
            TmdbOptions {
                enabled: true,
                api_key: Some("test-key".into()),
                cache_dir: dir.join("tmdb-cache"),
                ..TmdbOptions::default()
            },
            fetch,
        )
    }

    /// TMDB returns a poster (pre-seeded in the artwork cache): the DTO carries
    /// only its hashed URL and the IMDb fetch is never attempted.
    #[tokio::test]
    async fn tmdb_poster_wins_and_imdb_is_not_called() {
        let dir = std::env::temp_dir().join(format!("mh-tmdb-win-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");
        let poster_path = "/abc123def.jpg";
        let key = media_metadata::artwork_key(poster_path);
        std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");

        let imdb_calls = Arc::new(AtomicUsize::new(0));
        let calls = imdb_calls.clone();
        let imdb = ImdbArtworkProvider::with_fetch(Box::new(move |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(FAKE_URL.into())
        }));
        let owned_path = poster_path.to_string();
        let tmdb = fake_tmdb(&dir, Box::new(move |_, _, _| Ok(tmdb_json(&owned_path))));
        let state = AppState::new(service_for_test().await, "test-token")
            .with_tmdb(tmdb)
            .with_imdb_artwork(imdb)
            .with_artwork_dir(artwork_dir);

        let items = enrich_artwork(&state, vec![summary_for_test()]).await;
        let refs = &items[0].artwork;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].variant, ArtworkVariant::Poster);
        let expected = format!("/artwork/{key}");
        assert_eq!(refs[0].url.as_deref(), Some(expected.as_str()));
        assert_eq!(
            imdb_calls.load(Ordering::SeqCst),
            0,
            "IMDb must not be called"
        );
        let json = serde_json::to_string(&items[0]).unwrap();
        assert!(!json.contains("image.tmdb.org"));
        assert!(!json.contains("abc123def"));
        assert!(!json.contains("m.media-amazon.com"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// TMDB fails (no usable poster): the IMDb block runs and supplies the URL.
    #[tokio::test]
    async fn tmdb_failure_falls_back_to_imdb() {
        let dir = std::env::temp_dir().join(format!("mh-tmdb-fallback-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");
        let key = media_metadata::artwork_key(FAKE_URL);
        std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");

        let imdb_calls = Arc::new(AtomicUsize::new(0));
        let calls = imdb_calls.clone();
        let imdb = ImdbArtworkProvider::with_fetch(Box::new(move |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(FAKE_URL.into())
        }));
        let tmdb = fake_tmdb(&dir, Box::new(|_, _, _| Err("boom".into())));
        let state = AppState::new(service_for_test().await, "test-token")
            .with_tmdb(tmdb)
            .with_imdb_artwork(imdb)
            .with_artwork_dir(artwork_dir);

        let items = enrich_artwork(&state, vec![summary_for_test()]).await;
        let refs = &items[0].artwork;
        assert_eq!(refs.len(), 1);
        let expected = format!("/artwork/{key}");
        assert_eq!(refs[0].url.as_deref(), Some(expected.as_str()));
        assert_eq!(imdb_calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Both providers fail: no artwork is added and bundled art is untouched.
    #[tokio::test]
    async fn both_providers_failing_leaves_artwork_empty() {
        let dir = std::env::temp_dir().join(format!("mh-tmdb-none-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");

        let imdb_calls = Arc::new(AtomicUsize::new(0));
        let calls = imdb_calls.clone();
        let imdb = ImdbArtworkProvider::with_fetch(Box::new(move |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Err("boom".into())
        }));
        let tmdb = fake_tmdb(&dir, Box::new(|_, _, _| Err("boom".into())));
        let state = AppState::new(service_for_test().await, "test-token")
            .with_tmdb(tmdb)
            .with_imdb_artwork(imdb)
            .with_artwork_dir(artwork_dir);

        let items = enrich_artwork(&state, vec![summary_for_test()]).await;
        assert!(items[0].artwork.is_empty());
        assert_eq!(imdb_calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Task 2: discover_catalog ------------------------------------------
    // Handlers are called directly (not via a test router): the route-parity
    // test counts route registrations in this file, so tests add none.

    /// A fake TMDB discover page. The poster path is the only upstream detail
    /// the provider keeps; the DTO must expose neither it nor the host or key.
    const DISCOVER_JSON: &str = r#"{"page":1,"results":[
      {"id":7,"title":"A Film","release_date":"2020-05-01","overview":"A story.","poster_path":"/p1.jpg","vote_average":7.2}
    ]}"#;

    const FAKE_DISCOVER_KEY: &str = "SENTINEL-DISCOVER-KEY";

    fn discover_options(dir: &std::path::Path, enabled: bool) -> DiscoverOptions {
        DiscoverOptions {
            enabled,
            api_key: Some(FAKE_DISCOVER_KEY.to_string()),
            cache_dir: dir.join("discover-cache"),
        }
    }

    fn args(kind: &str, page: u32) -> ApiJson<DiscoverArgs> {
        ApiJson(DiscoverArgs {
            kind: kind.to_string(),
            page,
        })
    }

    /// Drives the real handler with fakes and a pre-seeded JPEG: no network,
    /// no image work, and the DTO carries only the hashed URL.
    #[tokio::test]
    async fn discover_catalog_emits_only_relative_hashed_poster_url() {
        let dir = std::env::temp_dir().join(format!("mh-discover-route-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");
        let poster_path = "/p1.jpg";
        let key = media_metadata::artwork_key(poster_path);
        std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");

        let calls = Arc::new(AtomicUsize::new(0));
        let fetch_calls = calls.clone();
        let provider = DiscoverProvider::with_fetch(
            discover_options(&dir, true),
            Box::new(move |_, _, supplied_key| {
                fetch_calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(supplied_key, FAKE_DISCOVER_KEY);
                Ok(DISCOVER_JSON.to_string())
            }),
        );
        let state = AppState::new(service_for_test().await, "test-token")
            .with_discover(provider)
            .with_artwork_dir(artwork_dir);

        let screen = discover_catalog(State(state), args("movie", 1))
            .await
            .ok()
            .expect("discover succeeds")
            .0;
        let json = serde_json::to_string(&screen).expect("serialise DTO");
        assert!(json.contains("\"available\":true"), "DTO: {json}");
        let expected = format!("/artwork/{key}");
        assert!(
            json.contains(&expected),
            "DTO must carry the hashed URL: {json}"
        );
        assert!(
            !json.contains("image.tmdb.org"),
            "DTO leaked the host: {json}"
        );
        assert!(
            !json.contains(poster_path),
            "DTO leaked the poster path: {json}"
        );
        assert!(
            !json.contains(FAKE_DISCOVER_KEY),
            "DTO leaked the API key: {json}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// No key means no fetch, no image read and an empty, unavailable screen.
    #[tokio::test]
    async fn discover_catalog_without_a_key_does_no_fetch() {
        let dir = std::env::temp_dir().join(format!("mh-discover-off-{}", uuid::Uuid::new_v4()));
        let calls = Arc::new(AtomicUsize::new(0));
        let fetch_calls = calls.clone();
        let provider = DiscoverProvider::with_fetch(
            DiscoverOptions {
                enabled: false,
                api_key: None,
                cache_dir: dir.join("discover-cache"),
            },
            Box::new(move |_, _, _| {
                fetch_calls.fetch_add(1, Ordering::SeqCst);
                Ok(DISCOVER_JSON.to_string())
            }),
        );
        let state = AppState::new(service_for_test().await, "test-token").with_discover(provider);

        let screen = discover_catalog(State(state), args("movie", 1))
            .await
            .ok()
            .expect("discover succeeds")
            .0;
        assert!(!screen.available);
        assert!(screen.entries.is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "no key must mean zero fetch"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Unknown kinds are refused before any provider or artwork work.
    #[tokio::test]
    async fn discover_catalog_rejects_unknown_kind() {
        let dir = std::env::temp_dir().join(format!("mh-discover-kind-{}", uuid::Uuid::new_v4()));
        let provider = DiscoverProvider::with_fetch(
            discover_options(&dir, true),
            Box::new(|_, _, _| Ok(DISCOVER_JSON.to_string())),
        );
        let state = AppState::new(service_for_test().await, "test-token").with_discover(provider);

        let error = discover_catalog(State(state), args("other", 1))
            .await
            .expect_err("unknown kind must be refused");
        assert_eq!(error.0.code, AppErrorCode::InvalidRequest);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Pages are clamped into 1..=20 before reaching the provider.
    #[tokio::test]
    async fn discover_catalog_clamps_the_page() {
        let dir = std::env::temp_dir().join(format!("mh-discover-page-{}", uuid::Uuid::new_v4()));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let pages = seen.clone();
        let provider = DiscoverProvider::with_fetch(
            discover_options(&dir, true),
            Box::new(move |_, page, _| {
                pages.lock().expect("lock").push(page);
                Ok(DISCOVER_JSON.to_string())
            }),
        );
        let state = AppState::new(service_for_test().await, "test-token").with_discover(provider);

        for page in [0, 999] {
            let _ = discover_catalog(State(state.clone()), args("movie", page))
                .await
                .ok()
                .expect("discover succeeds");
        }
        assert_eq!(*seen.lock().expect("lock"), vec![1, 20]);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A cold 20-item catalogue must not stall the response: poster downloads
    /// are capped, but every entry is still returned (the rest are display-only
    /// with no artwork). Posters are pre-seeded so the fake makes no download.
    #[tokio::test]
    async fn discover_catalog_caps_posters_but_returns_every_entry() {
        let dir = std::env::temp_dir().join(format!("mh-discover-cap-{}", uuid::Uuid::new_v4()));
        let artwork_dir = dir.join("artwork");
        std::fs::create_dir_all(&artwork_dir).expect("create temp dir");

        let total = 20usize;
        let mut results = Vec::new();
        for index in 0..total {
            let poster_path = format!("/p{index}.jpg");
            let key = media_metadata::artwork_key(&poster_path);
            std::fs::write(artwork_dir.join(&key), [0xFF, 0xD8, 0xFF, 0x00]).expect("seed poster");
            results.push(format!(
                r#"{{"id":{},"title":"Film {}","release_date":"2020-05-01","poster_path":"{}"}}"#,
                index + 1,
                index,
                poster_path
            ));
        }
        let page_json = format!(r#"{{"page":1,"results":[{}]}}"#, results.join(","));

        let provider = DiscoverProvider::with_fetch(
            discover_options(&dir, true),
            Box::new(move |_, _, _| Ok(page_json.clone())),
        );
        let state = AppState::new(service_for_test().await, "test-token")
            .with_discover(provider)
            .with_artwork_dir(artwork_dir);

        let screen = discover_catalog(State(state), args("movie", 1))
            .await
            .ok()
            .expect("discover succeeds")
            .0;
        assert_eq!(
            screen.entries.len(),
            total,
            "no entry may be dropped or truncated"
        );
        let with_art = screen
            .entries
            .iter()
            .filter(|entry| !entry.artwork.is_empty())
            .count();
        assert_eq!(with_art, MAX_DISCOVER_POSTERS_PER_REQUEST);
        std::fs::remove_dir_all(&dir).ok();
    }
}
