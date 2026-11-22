//! MEDIA_HAVEN Windows host.
//!
//! The host owns the window, the container supervisor, the per-install token
//! and the media player. The renderer reaches it only through named commands
//! (ADR-0005); there is no generic passthrough command.

mod commands;
mod mounts;
mod player;
mod secret;
mod supervisor;
mod wsl;

use serde::Serialize;

pub use mounts::{render_mounts, write_atomic, MountError, RenderOutput, RenderedMount, RootSpec};
pub use secret::{blob_path, load_or_create_token, SecretError, SecretToken};
pub use supervisor::{Supervisor, SupervisorStatus};
pub use wsl::Wsl;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub os: &'static str,
    pub arch: &'static str,
}

/// Smoke-test command for Task A2. It exposes no path, token or host detail.
#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(player::PlayerState::new())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            commands::runtime_start,
            commands::library_home,
            commands::library_movies,
            commands::library_shows,
            commands::library_detail,
            commands::library_episodes,
            commands::library_search,
            commands::library_roots,
            commands::library_root_add,
            commands::library_root_remove,
            commands::library_scan,
            commands::library_watchlist,
            commands::watchlist_add,
            commands::watchlist_remove,
            commands::watchlist_reorder,
            commands::queue_list,
            commands::queue_add,
            commands::queue_remove,
            commands::queue_reorder,
            commands::downloads_list,
            commands::discover_catalog,
            commands::download_add,
            commands::source_set,
            commands::download_status,
            commands::download_remove,
            commands::download_root_get,
            commands::download_root_set,
            commands::imdb_lookup,
            commands::indexers_seed,
            commands::source_search,
            commands::settings_get,
            commands::settings_set,
            commands::playback_start,
            commands::playback_stop,
            commands::playback_checkpoint,
            commands::player_control,
            commands::provider_secret_set,
            commands::provider_secret_has,
            commands::provider_secret_clear,
        ])
        .run(tauri::generate_context!())
        .expect("the MEDIA_HAVEN window failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_this_platform() {
        let info = get_app_info();
        assert_eq!(info.os, "windows");
        assert_eq!(info.arch, "x86_64");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
