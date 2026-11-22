//! Process wiring: config from the environment, the router, and the listener.
//!
//! The listener binds loopback by default. A container needs `0.0.0.0` inside
//! its own network namespace, which the operator sets explicitly via
//! `MEDIAHAVEN_BIND`; the host publishes it on `127.0.0.1` only (ADR-0005 §1).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context};
use app_core::application::LibraryService;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::middleware;
use axum::Router;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::routes::{protected_router, public_router};
use crate::state::AppState;

/// Token used only when `MEDIAHAVEN_DEV=1` and no token is supplied.
pub const DEV_TOKEN: &str = "dev-only-not-for-production";

const DEFAULT_BIND: &str = "127.0.0.1:8080";

/// Minimum token length outside development (ADR-0005: 256 bits from the CSPRNG).
const MIN_TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
}

impl Config {
    /// Resolve configuration from the environment.
    ///
    /// Secrets are never logged and never defaulted outside development.
    pub fn from_env() -> anyhow::Result<Self> {
        let dev = std::env::var("MEDIAHAVEN_DEV").as_deref() == Ok("1");

        let token = match std::env::var("MEDIAHAVEN_API_TOKEN") {
            Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
            _ if dev => {
                tracing::warn!(
                    "MEDIAHAVEN_DEV=1: using the development API token. \
                     This is not for production."
                );
                DEV_TOKEN.to_owned()
            }
            _ => bail!(
                "MEDIAHAVEN_API_TOKEN is not set. Provide the install token, \
                 or set MEDIAHAVEN_DEV=1 for local development."
            ),
        };

        if !dev && token.len() < MIN_TOKEN_BYTES {
            bail!("MEDIAHAVEN_API_TOKEN must be at least {MIN_TOKEN_BYTES} characters.");
        }

        let raw_bind = std::env::var("MEDIAHAVEN_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
        let bind: SocketAddr = raw_bind
            .parse()
            .with_context(|| format!("MEDIAHAVEN_BIND is not a socket address: {raw_bind}"))?;

        Ok(Self {
            token,
            bind,
            data_dir: default_data_dir(),
        })
    }
}

/// `MEDIAHAVEN_DATA_DIR`, else the platform data directory for this user.
pub fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MEDIAHAVEN_DATA_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    if let Ok(base) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(base).join("MEDIA_HAVEN");
    }
    if let Ok(base) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(base).join("mediahaven");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/mediahaven");
    }
    PathBuf::from(".mediahaven")
}

/// Assemble the full route table.
pub fn build_router(state: AppState) -> Router {
    // Host-internal acquisition routes share the signed auth middleware but
    // stay outside the renderer command surface (see acquisition.rs).
    let protected = protected_router()
        .merge(crate::acquisition::host_router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    public_router()
        .merge(protected)
        .layer(DefaultBodyLimit::max(auth::MAX_BODY_BYTES))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Only the dev server needs cross-origin access, and only for loopback origins.
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            is_loopback_origin(origin)
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static(auth::HEADER_TIMESTAMP),
            HeaderName::from_static(auth::HEADER_SIGNATURE),
        ])
        .max_age(Duration::from_secs(600))
}

fn is_loopback_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .unwrap_or(origin);
    let authority = rest.split('/').next().unwrap_or(rest);
    let host = match authority.rsplit_once(':') {
        Some((host, _port)) => host,
        None => authority,
    };
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// Bind, serve, and never return on success.
pub async fn run(config: Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir).with_context(|| {
        format!(
            "could not create the data directory {}",
            config.data_dir.display()
        )
    })?;

    let database = config.data_dir.join("library.db");
    let pool = connect_path(&database)
        .await
        .with_context(|| format!("could not open {}", database.display()))?;
    crate::seed::seed_if_requested(&pool).await?;
    let service = LibraryService::new(SqliteLibraryRepository::new(pool));
    let recovered = service.recover_interrupted_scans().await?;
    if recovered > 0 {
        tracing::warn!(recovered, "marked interrupted library scans as errors");
    }
    let artwork_dir = config.data_dir.join("artwork");
    let state = AppState::new(service, config.token)
        .with_imdb(crate::state::imdb_provider_from_env())
        .with_tmdb(crate::state::tmdb_provider_from_env(&config.data_dir))
        .with_artwork_dir(artwork_dir);

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("could not bind {}", config.bind))?;

    tracing::info!(
        address = %config.bind,
        data_dir = %config.data_dir.display(),
        "mediahaven-core listening"
    );

    axum::serve(listener, app)
        .await
        .context("the HTTP server stopped unexpectedly")?;
    Ok(())
}
