//! Repository behaviour against a real (temp) SQLite file.
//!
//! The SQL is only validated when it runs, so these tests exercise every query
//! the HTTP layer will call rather than relying on the migration test alone.

use app_core::domain::*;
use app_core::ports::LibraryRepository;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};
use sqlx::Row;

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
    assert_eq!(
        fixture.repo.movies(&filtered).await.expect("genre").len(),
        1
    );
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
    assert!(home
        .shelves
        .iter()
        .any(|s| s.id == ShelfId::RecentlyAddedMovies));

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
            profiles: None,
        })
        .await
        .expect("settings set");
    assert!(!updated.playback.autoplay_next);

    // Round-trips through the settings row rather than the default.
    let reloaded = fixture.repo.settings_get().await.expect("settings get");
    assert!(!reloaded.playback.autoplay_next);
}

#[tokio::test]
async fn profiles_store_real_colours_not_css_variables() {
    let fixture = Fixture::new().await;
    let created_at = "2026-11-16T00:00:00+00:00";
    let profile = |id: &str, colour: &str| Profile {
        id: id.to_string(),
        name: id.to_string(),
        avatar_color: colour.to_string(),
        created_at: created_at.to_string(),
    };

    let stored = fixture
        .repo
        .settings_set(&SettingsPatch {
            playback: None,
            storage: None,
            profiles: Some(vec![
                profile("assigned", ""),
                profile("invalid", "red"),
                profile("hex", "#ABC123"),
            ]),
        })
        .await
        .expect("settings set");

    assert_eq!(stored.profiles[0].avatar_color, "#5f7183");
    assert_eq!(stored.profiles[1].avatar_color, "#7d6a7a");
    assert_eq!(stored.profiles[2].avatar_color, "#abc123");

    let reloaded = fixture.repo.settings_get().await.expect("settings get");
    for profile in &reloaded.profiles {
        assert!(
            profile.avatar_color.starts_with('#') && profile.avatar_color.len() == 7,
            "stored colour is not a hex value: {}",
            profile.avatar_color
        );
    }
    assert_eq!(reloaded.profiles[2].avatar_color, "#abc123");
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
    assert!(fixture
        .repo
        .watchlist()
        .await
        .expect("list")
        .entries
        .is_empty());

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
    fixture
        .repo
        .queue_remove(&queue_id)
        .await
        .expect("queue remove");
    assert!(fixture
        .repo
        .queue()
        .await
        .expect("queue list")
        .entries
        .is_empty());

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
    assert!(home
        .shelves
        .iter()
        .any(|s| s.id == ShelfId::ContinueWatching));

    // Reopening without an explicit start must continue from the checkpoint
    // rather than resetting the resume position to zero.
    let resumed = fixture
        .repo
        .playback_start(&PlaybackStartRequest {
            media_id: "m1".to_string(),
            start_position_seconds: None,
        })
        .await
        .expect("resume");
    assert_eq!(resumed.start_position_seconds, 120.0);

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

#[tokio::test]
async fn rescan_preserves_watchlist_and_queue() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let items = vec![MediaUpsert {
        metadata: None,
        id: "m1".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h".to_string()),
    }];
    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("scan");
    fixture.repo.watchlist_add("m1").await.expect("wl");
    fixture.repo.queue_add("m1").await.expect("q");
    let wl_before = fixture.repo.watchlist().await.unwrap().entries.len();
    let q_before = fixture.repo.queue().await.unwrap().entries.len();
    let created_before: String =
        sqlx::query_scalar("SELECT created_at FROM media_items WHERE id = 'm1'")
            .fetch_one(fixture.repo.pool())
            .await
            .unwrap();

    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("rescan");
    assert_eq!(
        fixture.repo.watchlist().await.unwrap().entries.len(),
        wl_before
    );
    assert_eq!(fixture.repo.queue().await.unwrap().entries.len(), q_before);
    let created_after: String =
        sqlx::query_scalar("SELECT created_at FROM media_items WHERE id = 'm1'")
            .fetch_one(fixture.repo.pool())
            .await
            .unwrap();
    assert_eq!(created_before, created_after);
}

#[tokio::test]
async fn rescan_preserves_progress_and_availability() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let items = vec![MediaUpsert {
        metadata: None,
        id: "m1".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h".to_string()),
    }];
    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("scan");
    sqlx::query("UPDATE media_items SET availability_state = 'missing-file', availability_last_seen = '2026-11-16T00:00:00+00:00', progress_position = 42.0, progress_duration = 100.0, progress_completed = 0, progress_updated_at = '2026-11-16T00:00:00+00:00' WHERE id = 'm1'")
        .execute(fixture.repo.pool()).await.unwrap();

    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("rescan");

    let row: (String, Option<f64>) = sqlx::query_as(
        "SELECT availability_state, progress_position FROM media_items WHERE id = 'm1'",
    )
    .fetch_one(fixture.repo.pool())
    .await
    .unwrap();
    assert_eq!(row.0, "missing-file");
    assert_eq!(row.1, Some(42.0));
}

#[tokio::test]
async fn prune_removes_only_stale_ids() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let mk = |id: &str| MediaUpsert {
        metadata: None,
        id: id.to_string(),
        kind: MediaKind::Movie,
        title: id.to_string(),
        root_id: None,
        path: Some(format!("/tmp/r/{id}.mkv")),
        size_bytes: Some(1),
        hash: Some("h".to_string()),
    };
    fixture
        .repo
        .media_replace_for_root(&root.id, &[mk("a"), mk("b")])
        .await
        .expect("scan1");
    fixture
        .repo
        .media_replace_for_root(&root.id, &[mk("a"), mk("c")])
        .await
        .expect("scan2");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE root_id = ?")
        .bind(&root.id)
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 2);

    let b: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE id = 'b'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(b, 0);
    let a: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE id = 'a'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(a, 1);
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE id = 'c'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(c, 1);
}

#[tokio::test]
async fn rescan_does_not_accumulate_media_files_rows() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let items = vec![MediaUpsert {
        metadata: None,
        id: "m1".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h".to_string()),
    }];
    for _ in 0..3 {
        fixture
            .repo
            .media_replace_for_root(&root.id, &items)
            .await
            .expect("scan");
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE media_id = 'm1'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn empty_id_is_rejected() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let items = vec![MediaUpsert {
        metadata: None,
        id: String::new(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h".to_string()),
    }];
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    let res = fixture.repo.media_replace_for_root(&root.id, &items).await;
    assert!(res.is_err());
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn search_treats_like_metacharacters_literally() {
    let fixture = Fixture::new().await;
    // insert directly
    let now = "2026-11-16T00:00:00+00:00";
    for title in ["100% Pure", "a_b", "a%b"] {
        sqlx::query("INSERT INTO media_items (id, kind, title, created_at, updated_at) VALUES (?, 'movie', ?, ?, ?)")
            .bind(format!("m-{}", title))
            .bind(title)
            .bind(now)
            .bind(now)
            .execute(fixture.repo.pool()).await.unwrap();
    }

    let pct = fixture
        .repo
        .search(&SearchQuery {
            text: "%".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .unwrap();
    // Only titles containing a literal '%' match; '%' is not a wildcard.
    assert_eq!(pct.items.len(), 2);

    let us = fixture
        .repo
        .search(&SearchQuery {
            text: "_".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .unwrap();
    assert_eq!(us.items.len(), 1);
    assert_eq!(us.items[0].title, "a_b");

    let hundred = fixture
        .repo
        .search(&SearchQuery {
            text: "100".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .unwrap();
    assert_eq!(hundred.items.len(), 1);
    assert_eq!(hundred.items[0].title, "100% Pure");
}

#[tokio::test]
async fn search_query_uses_escape_clause() {
    let fixture = Fixture::new().await;
    let now = "2026-11-16T00:00:00+00:00";
    for title in ["100% Pure", "100X Pure"] {
        sqlx::query("INSERT INTO media_items (id, kind, title, created_at, updated_at) VALUES (?, 'movie', ?, ?, ?)")
            .bind(format!("m-{}", title))
            .bind(title)
            .bind(now)
            .bind(now)
            .execute(fixture.repo.pool()).await.unwrap();
    }
    // Searching literal "%" should match only the title containing a percent.
    let res = fixture
        .repo
        .search(&SearchQuery {
            text: "%".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .unwrap();
    assert_eq!(res.items.len(), 1);
    assert_eq!(res.items[0].title, "100% Pure");
}

#[tokio::test]
async fn timestamps_round_trip_as_utc() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let items = vec![MediaUpsert {
        metadata: None,
        id: "m1".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h".to_string()),
    }];
    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("scan");
    let created: String = sqlx::query_scalar("SELECT created_at FROM media_items WHERE id = 'm1'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    let updated: String = sqlx::query_scalar("SELECT updated_at FROM media_items WHERE id = 'm1'")
        .fetch_one(fixture.repo.pool())
        .await
        .unwrap();
    let c = chrono::DateTime::parse_from_rfc3339(&created).unwrap();
    let u = chrono::DateTime::parse_from_rfc3339(&updated).unwrap();
    assert_eq!(c.offset().local_minus_utc(), 0);
    assert_eq!(u.offset().local_minus_utc(), 0);
    assert!((c - u).num_seconds().abs() <= 1);
}

#[tokio::test]
async fn browse_query_plan_uses_the_root_index() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    let rows = sqlx::query("EXPLAIN QUERY PLAN SELECT id FROM media_items WHERE root_id = 'r1'")
        .fetch_all(fixture.repo.pool())
        .await
        .unwrap();
    let mut detail = String::new();
    for r in &rows {
        let d: String = r.get("detail");
        detail.push_str(&d);
        detail.push(' ');
    }
    assert!(!detail.contains("SCAN media_items"), "plan: {detail}");
    assert!(
        detail.contains("idx_media_items_root")
            || detail.contains("sqlite_autoindex")
            || detail.contains("INDEX"),
        "plan: {detail}"
    );
}

#[tokio::test]
#[ignore]
async fn searching_10k_items_is_within_budget() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");
    let mut items = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        items.push(MediaUpsert {
            metadata: None,
            id: format!("m{i}"),
            kind: MediaKind::Movie,
            title: format!("item-{i:05}"),
            root_id: None,
            path: Some(format!("/tmp/r/{i}.mkv")),
            size_bytes: Some(1),
            hash: Some("h".to_string()),
        });
    }
    // one item with a needle
    items[5000].title = "needle-title".to_string();
    fixture
        .repo
        .media_replace_for_root(&root.id, &items)
        .await
        .expect("scan");

    let start = std::time::Instant::now();
    let res = fixture
        .repo
        .search(&SearchQuery {
            text: "needle".to_string(),
            kinds: vec![],
            genres: vec![],
        })
        .await
        .unwrap();
    let elapsed = start.elapsed();
    assert!(!res.items.is_empty());
    assert!(
        elapsed.as_millis() < 200,
        "search took {} ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn playback_checkpoint_writes_progress() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    fixture
        .repo
        .playback_checkpoint(&PlaybackCheckpointRequest {
            media_id: "m1".to_string(),
            position_seconds: 42.0,
            duration_seconds: Some(100.0),
        })
        .await
        .expect("checkpoint");
    let pos: Option<f64> =
        sqlx::query_scalar("SELECT progress_position FROM media_items WHERE id = 'm1'")
            .fetch_one(fixture.repo.pool())
            .await
            .unwrap();
    assert_eq!(pos, Some(42.0));
    let dur: Option<f64> =
        sqlx::query_scalar("SELECT progress_duration FROM media_items WHERE id = 'm1'")
            .fetch_one(fixture.repo.pool())
            .await
            .unwrap();
    assert_eq!(dur, Some(100.0));

    let err = fixture
        .repo
        .playback_checkpoint(&PlaybackCheckpointRequest {
            media_id: "ghost".to_string(),
            position_seconds: 1.0,
            duration_seconds: None,
        })
        .await
        .expect_err("missing media");
    assert_eq!(err.code, AppErrorCode::MediaMissing);
}

#[tokio::test]
async fn scanned_sidecar_metadata_is_stored_and_survives_a_bare_rescan() {
    let fixture = Fixture::new().await;
    let root = fixture
        .repo
        .root_add("Movies", "/tmp/r")
        .await
        .expect("root");

    let with_sidecar = MediaUpsert {
        metadata: Some(MetadataRecord {
            title: Some("Arrival".to_string()),
            year: Some(2016),
            synopsis: Some("A linguist is recruited to translate.".to_string()),
            runtime_minutes: Some(116),
            genres: vec!["Science Fiction".to_string(), "Drama".to_string()],
        }),
        id: "m-nfo".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/Arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h-nfo".to_string()),
    };
    fixture
        .repo
        .media_replace_for_root(&root.id, &[with_sidecar])
        .await
        .expect("scan");

    match fixture.repo.detail("m-nfo").await.expect("detail") {
        MediaDetail::Movie(movie) => {
            assert_eq!(movie.year, Some(2016));
            assert_eq!(
                movie.synopsis.as_deref(),
                Some("A linguist is recruited to translate.")
            );
            assert_eq!(movie.runtime_minutes, Some(116));
            assert_eq!(
                movie.genres,
                vec!["Science Fiction".to_string(), "Drama".to_string()]
            );
        }
        MediaDetail::Series(_) => panic!("m-nfo must be a movie"),
    }

    // The sidecar is gone on the next scan (file renamed, drive offline, or the
    // user deleted it). Stored metadata must survive rather than be blanked.
    let bare = MediaUpsert {
        metadata: None,
        id: "m-nfo".to_string(),
        kind: MediaKind::Movie,
        title: "Arrival".to_string(),
        root_id: None,
        path: Some("/tmp/r/Arrival.mkv".to_string()),
        size_bytes: Some(10),
        hash: Some("h-nfo".to_string()),
    };
    fixture
        .repo
        .media_replace_for_root(&root.id, &[bare])
        .await
        .expect("rescan");

    match fixture
        .repo
        .detail("m-nfo")
        .await
        .expect("detail after rescan")
    {
        MediaDetail::Movie(movie) => {
            assert_eq!(movie.year, Some(2016));
            assert_eq!(
                movie.synopsis.as_deref(),
                Some("A linguist is recruited to translate.")
            );
            assert_eq!(movie.runtime_minutes, Some(116));
            assert_eq!(movie.genres.len(), 2);
        }
        MediaDetail::Series(_) => panic!("m-nfo must be a movie"),
    }
}

// --- C2: durable queue semantics -------------------------------------------

async fn seed_queue(fixture: &Fixture, media_ids: &[&str]) -> Vec<String> {
    for id in media_ids {
        fixture.repo.queue_add(id).await.expect("queue add");
    }
    fixture
        .repo
        .queue()
        .await
        .expect("queue list")
        .entries
        .iter()
        .map(|e| e.id.clone())
        .collect()
}

#[tokio::test]
async fn queue_reorder_is_atomic_and_renumbers_densely() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    let ids = seed_queue(&fixture, &["m1", "s1", "e1"]).await;
    let reversed: Vec<String> = ids.iter().rev().cloned().collect();

    fixture
        .repo
        .queue_reorder(&reversed)
        .await
        .expect("queue reorder");

    let after = fixture.repo.queue().await.expect("queue list");
    assert_eq!(after.entries.len(), 3, "reorder dropped an entry");
    for (i, entry) in after.entries.iter().enumerate() {
        assert_eq!(entry.position, i as i64, "positions are not dense");
        assert_eq!(entry.id, reversed[i], "order not applied");
        // A reorder never advances state on its own.
        assert_eq!(entry.state, QueueEntryState::Pending);
    }
}

#[tokio::test]
async fn queue_reorder_rejects_unknown_duplicate_and_incomplete_orders() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    let ids = seed_queue(&fixture, &["m1", "s1"]).await;
    let before = fixture.repo.queue().await.expect("queue list");

    let duplicate = vec![ids[0].clone(), ids[0].clone()];
    assert_eq!(
        fixture
            .repo
            .queue_reorder(&duplicate)
            .await
            .expect_err("duplicate id")
            .code,
        AppErrorCode::InvalidRequest
    );

    let unknown = vec![ids[0].clone(), "ghost".to_string()];
    assert_eq!(
        fixture
            .repo
            .queue_reorder(&unknown)
            .await
            .expect_err("unknown id")
            .code,
        AppErrorCode::InvalidRequest
    );

    let incomplete = &ids[..1];
    assert_eq!(
        fixture
            .repo
            .queue_reorder(incomplete)
            .await
            .expect_err("partial order")
            .code,
        AppErrorCode::InvalidRequest
    );

    // Every rejected reorder must leave the queue exactly as it was.
    let after = fixture.repo.queue().await.expect("queue list");
    assert_eq!(after.entries.len(), before.entries.len());
    for (a, b) in after.entries.iter().zip(before.entries.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.position, b.position);
        assert_eq!(a.state, b.state);
    }
}

#[tokio::test]
async fn forced_kill_before_commit_leaves_order_consistent() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    let ids = seed_queue(&fixture, &["m1", "s1", "e1"]).await;
    let before = fixture.repo.queue().await.expect("queue list");

    // Emulate a forced kill mid-reorder: write new positions inside a
    // transaction that is dropped before commit. SQLite must roll it back.
    {
        let mut tx = fixture.repo.pool().begin().await.expect("begin");
        for (position, id) in ids.iter().rev().enumerate() {
            sqlx::query("UPDATE queue_items SET position = ? WHERE id = ?")
                .bind(position as i64)
                .bind(id)
                .execute(&mut *tx)
                .await
                .expect("write");
        }
        // No commit: dropping the transaction rolls it back, like a crash.
    }

    let after = fixture.repo.queue().await.expect("queue list");
    assert_eq!(after.entries.len(), 3);
    for (a, b) in after.entries.iter().zip(before.entries.iter()) {
        assert_eq!(a.id, b.id, "order changed after rolled-back reorder");
        assert_eq!(a.position, b.position);
        assert_eq!(a.state, b.state);
    }
}

#[tokio::test]
async fn queue_remove_archives_to_history_without_advancing() {
    let fixture = Fixture::new().await;
    fixture.seed().await;
    let ids = seed_queue(&fixture, &["m1", "s1"]).await;

    fixture
        .repo
        .queue_remove(&ids[0])
        .await
        .expect("queue remove");

    let remaining = fixture.repo.queue().await.expect("queue list");
    assert_eq!(remaining.entries.len(), 1);
    assert_eq!(remaining.entries[0].id, ids[1]);
    assert_eq!(remaining.entries[0].state, QueueEntryState::Pending);
    // queue_add seeds 1-based positions; remove must not silently renumber.
    assert_eq!(remaining.entries[0].position, 2);

    let (media_id, state, position): (String, String, i64) =
        sqlx::query_as("SELECT media_id, state, position FROM queue_history WHERE queue_id = ?")
            .bind(&ids[0])
            .fetch_one(fixture.repo.pool())
            .await
            .expect("history row");
    assert_eq!(media_id, "m1");
    assert_eq!(state, "pending");
    assert_eq!(position, 1);
}
