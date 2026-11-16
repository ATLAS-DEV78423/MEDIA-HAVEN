//! Repository behaviour against a real (temp) SQLite file.
//!
//! The SQL is only validated when it runs, so these tests exercise every query
//! the HTTP layer will call rather than relying on the migration test alone.

use app_core::domain::*;
use app_core::ports::LibraryRepository;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};

struct Fixture {
    dir: std::path::PathBuf,
    repo: SqliteLibraryRepository,
}

impl Fixture {
    async fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mediahaven-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect");
        Self {
            dir,
            repo: SqliteLibraryRepository::new(pool),
        }
    }

    async fn seed(&self) {
        let pool = self.repo.pool();
        let now = "2026-11-16T00:00:00+00:00";
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, year, genres, created_at, updated_at) \
             VALUES ('m1', 'movie', 'Arrival', 2016, '[\"sci-fi\"]', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert movie");

        sqlx::query(
            "INSERT INTO media_items (id, kind, title, year, created_at, updated_at) \
             VALUES ('s1', 'series', 'Severance', 2022, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert series");

        for (id, season, episode, title) in [
            ("e1", 1_i64, 1_i64, "Good News About Hell"),
            ("e2", 1_i64, 2_i64, "Half Loop"),
        ] {
            sqlx::query(
                "INSERT INTO media_items \
                 (id, kind, title, series_id, season_number, episode_number, created_at, updated_at) \
                 VALUES (?, 'episode', ?, 's1', ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(title)
            .bind(season)
            .bind(episode)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .expect("insert episode");
        }

        sqlx::query(
            "INSERT INTO library_roots (id, display_name, item_count, status) \
             VALUES ('r1', 'Movies', 3, 'ok')",
        )
        .execute(pool)
        .await
        .expect("insert root");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn browse() -> BrowseQuery {
    BrowseQuery {
        sort: BrowseSort::Title,
        genre: None,
        limit: 50,
        offset: 0,
    }
}

#[tokio::test]
async fn browse_paths_return_seeded_media() {
    let fixture = Fixture::new().await;
    fixture.seed().await;

    let movies = fixture.repo.movies(&browse()).await.expect("movies");
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].title, "Arrival");
    assert_eq!(movies[0].genres, vec!["sci-fi".to_string()]);

    let shows = fixture.repo.shows(&browse()).await.expect("shows");
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].seasons.len(), 1);
    assert_eq!(shows[0].seasons[0].episode_count, 2);

    let episodes = fixture.repo.episodes("s1", 1).await.expect("episodes");
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].series_title, "Severance");
    assert_eq!(episodes[1].episode_number, 2);

    let filtered = BrowseQuery {
        genre: Some("sci-fi".to_string()),
        ..browse()
    };
    assert_eq!(fixture.repo.movies(&filtered).await.expect("genre").len(), 1);
}

#[tokio::test]
async fn detail_and_search_hit_the_right_rows() {
    let fixture = Fixture::new().await;
    fixture.seed().await;

    match fixture.repo.detail("m1").await.expect("movie detail") {
        MediaDetail::Movie(movie) => assert_eq!(movie.title, "Arrival"),
        MediaDetail::Series(_) => panic!("m1 must be a movie"),
    }
    match fixture.repo.detail("s1").await.expect("series detail") {
        MediaDetail::Series(series) => assert_eq!(series.seasons[0].number, 1),
        MediaDetail::Movie(_) => panic!("s1 must be a series"),
    }
    assert!(fixture.repo.detail("nope").await.is_err());

    let results = fixture
        .repo
        .search(&SearchQuery {
            text: "arr".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .expect("search");
    assert_eq!(results.items.len(), 1);
    assert!(!results.truncated);

    // LIKE metacharacters must be treated literally, not as wildcards.
    let wildcard = fixture
        .repo
        .search(&SearchQuery {
            text: "%".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .expect("search");
    assert!(wildcard.items.is_empty());
}

#[tokio::test]
async fn home_roots_and_settings_are_populated() {
    let fixture = Fixture::new().await;
    fixture.seed().await;

    let home = fixture.repo.home().await.expect("home");
    assert!(home.hero.is_some());
    assert!(home.shelves.iter().any(|s| s.id == ShelfId::RecentlyAddedMovies));

    let library = fixture.repo.roots().await.expect("roots");
    assert_eq!(library.roots.len(), 1);
    assert_eq!(library.counts.movies, 1);
    assert_eq!(library.counts.shows, 1);
    assert_eq!(library.counts.episodes, 2);

    let settings = fixture.repo.settings_get().await.expect("settings");
    assert_eq!(settings.library_roots.len(), 1);

    let updated = fixture
        .repo
        .settings_set(&SettingsPatch {
            playback: Some(PlaybackSettingsPatch {
                autoplay_next: Some(false),
                ..Default::default()
            }),
            storage: None,
        })
        .await
        .expect("settings set");
    assert!(!updated.playback.autoplay_next);

    // Round-trips through the settings row rather than the default.
    let reloaded = fixture.repo.settings_get().await.expect("settings get");
    assert!(!reloaded.playback.autoplay_next);
}

#[tokio::test]
async fn watchlist_and_queue_mutations_persist() {
    let fixture = Fixture::new().await;
    fixture.seed().await;

    fixture.repo.watchlist_add("m1").await.expect("add");
    // Adding twice must not duplicate the entry.
    fixture.repo.watchlist_add("m1").await.expect("re-add");
    let watchlist = fixture.repo.watchlist().await.expect("list");
    assert_eq!(watchlist.entries.len(), 1);
    assert_eq!(watchlist.entries[0].media.id, "m1");

    fixture
        .repo
        .watchlist_reorder(&["m1".to_string()])
        .await
        .expect("reorder");
    fixture.repo.watchlist_remove("m1").await.expect("remove");
    assert!(fixture.repo.watchlist().await.expect("list").entries.is_empty());

    fixture.repo.queue_add("m1").await.expect("queue add");
    let queue = fixture.repo.queue().await.expect("queue list");
    assert_eq!(queue.entries.len(), 1);
    assert_eq!(queue.entries[0].state, QueueEntryState::Pending);

    let queue_id = queue.entries[0].id.clone();
    fixture
        .repo
        .queue_reorder(std::slice::from_ref(&queue_id))
        .await
        .expect("queue reorder");
    fixture.repo.queue_remove(&queue_id).await.expect("queue remove");
    assert!(fixture.repo.queue().await.expect("queue list").entries.is_empty());

    assert!(fixture.repo.watchlist_add("ghost").await.is_err());
}

#[tokio::test]
async fn playback_start_records_progress_for_continue_watching() {
    let fixture = Fixture::new().await;
    fixture.seed().await;

    let session = fixture
        .repo
        .playback_start(&PlaybackStartRequest {
            media_id: "m1".to_string(),
            start_position_seconds: Some(120.0),
        })
        .await
        .expect("playback start");
    assert_eq!(session.start_position_seconds, 120.0);
    assert!(session.can_resume);

    let home = fixture.repo.home().await.expect("home");
    assert!(home.shelves.iter().any(|s| s.id == ShelfId::ContinueWatching));

    sqlx::query("UPDATE media_items SET availability_state = 'missing-file' WHERE id = 'm1'")
        .execute(fixture.repo.pool())
        .await
        .expect("mark missing");
    let error = fixture
        .repo
        .playback_start(&PlaybackStartRequest {
            media_id: "m1".to_string(),
            start_position_seconds: None,
        })
        .await
        .expect_err("missing media must not start");
    assert_eq!(error.code, AppErrorCode::MediaMissing);

    let downloads = fixture.repo.downloads().await.expect("downloads");
    assert!(!downloads.enabled);
}
