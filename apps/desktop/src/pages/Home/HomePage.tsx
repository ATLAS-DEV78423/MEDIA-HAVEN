import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { Hero } from "@/components/hero/Hero";
import { MediaCard } from "@/components/cards/MediaCard";
import { Shelf } from "@/components/shelf/Shelf";
import { buttonClass } from "@/components/ui/Controls";
import { EmptyState } from "@/components/ui/Surfaces";
import { CatalogCard } from "@/pages/Discover/CatalogCard";
import { toMedia } from "@/services/adapt";
import { api, type CatalogItem, type CatalogResult } from "@/services/api";
import { recommendBecauseYouWatched } from "@/services/recommend";
import { useAppState } from "@/state/AppState";
import styles from "./home.module.css";

const watchedLabel = (progress: number | undefined): string | undefined => {
  if (progress === undefined) return undefined;
  return `${Math.round(progress * 100)}% watched`;
};

export function HomePage() {
  const { data, activeProfile } = useAppState();
  const navigate = useNavigate();
  const [catalog, setCatalog] = useState<{ movies: CatalogItem[]; tv: CatalogItem[] }>({
    movies: [],
    tv: [],
  });
  const [catalogStale, setCatalogStale] = useState(false);
  const [playingId, setPlayingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const browse = () =>
      Promise.all([api.catalogBrowse("movie"), api.catalogBrowse("tv")]);
    const apply = (movies: CatalogResult, tv: CatalogResult) => {
      if (cancelled) return;
      // An unconfigured catalog makes zero host requests: keep any loaded shelves.
      if (movies.setup_required === true || tv.setup_required === true) return;
      setCatalog({ movies: movies.items ?? [], tv: tv.items ?? [] });
      setCatalogStale(false);
    };
    void (async () => {
      try {
        const [movies, tv] = await browse();
        apply(movies, tv);
      } catch {
        if (cancelled) return;
        // One bounded retry for transient throttling; no retry on setup_required.
        await new Promise<void>((resolve) => setTimeout(resolve, 800));
        if (cancelled) return;
        try {
          const [movies, tv] = await browse();
          apply(movies, tv);
        } catch {
          // Keep the last good shelves; the note appears only if we still have any.
          if (!cancelled) setCatalogStale(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const play = async (item: CatalogItem) => {
    setPlayingId(item.id);
    try {
      const { mediaId } = await api.catalogWantedAdd(item.title, item.year, item.kind);
      navigate(`/details/${mediaId}`);
    } catch {
      // No source is chosen here; Details remains the recovery surface.
    } finally {
      setPlayingId(null);
    }
  };

  if (!data) return null;

  const { home, watchlist, roots } = data;
  const heroMedia = home.hero
    ? { ...toMedia(home.hero.media), title: home.hero.headline || home.hero.media.title }
    : null;

  const shelves = home.shelves
    .map((shelf) => ({ id: shelf.id, title: shelf.title, items: shelf.items.map(toMedia) }))
    .filter((shelf) => shelf.items.length > 0);

  const saved = watchlist.entries.map((entry) => toMedia(entry.media));
  const recommendation = recommendBecauseYouWatched(data.movies.map(toMedia));
  const totalTitles = roots.counts.movies + roots.counts.shows;
  const empty =
    !heroMedia &&
    shelves.length === 0 &&
    saved.length === 0 &&
    catalog.movies.length === 0 &&
    catalog.tv.length === 0;

  return (
    <div className={`page ${styles.page}`}>
      {heroMedia ? <Hero slides={[heroMedia]} /> : null}

      <div className={styles.shelves}>
        {empty ? (
          <EmptyState
            title="This library is empty"
            body="The service is running, but no titles are indexed yet. Add a folder that holds your films or series and scan it. Nothing is copied or renamed."
            action={
              <Link to="/setup" className={buttonClass("solid")} data-focusable="true">
                Add a media folder
              </Link>
            }
          />
        ) : null}

        {catalog.movies.length > 0 ? (
          <Shelf title="Trending Movies">
            {catalog.movies.map((item) => (
              <CatalogCard
                key={`catalog-movie-${item.id}`}
                item={item}
                adding={playingId === item.id}
                onPlay={() => void play(item)}
              />
            ))}
          </Shelf>
        ) : null}

        {catalog.tv.length > 0 ? (
          <Shelf title="Trending TV">
            {catalog.tv.map((item) => (
              <CatalogCard
                key={`catalog-tv-${item.id}`}
                item={item}
                adding={playingId === item.id}
                onPlay={() => void play(item)}
              />
            ))}
          </Shelf>
        ) : null}

        {shelves.map((shelf, index) => (
          <Shelf
            key={shelf.id}
            title={shelf.title}
            {...(index === 0 && activeProfile
              ? { caption: `Picking up where ${activeProfile.name} left off` }
              : {})}
          >
            {shelf.items.map((media) => {
              const label = watchedLabel(media.progress);
              return (
                <MediaCard
                  key={`${shelf.id}-${media.id}`}
                  media={media}
                  variant={shelf.id === "continue-watching" ? "wide" : "still"}
                  {...(label ? { label } : {})}
                  {...(media.progress !== undefined ? { progress: media.progress } : {})}
                />
              );
            })}
          </Shelf>
        ))}

        {recommendation ? (
          <Shelf title={`Because you watched ${recommendation.seed.title}`}>
            {recommendation.picks.map((media) => (
              <MediaCard key={`because-you-watched-${media.id}`} media={media} />
            ))}
          </Shelf>
        ) : null}

        {saved.length > 0 ? (
          <Shelf
            title="My List"
            empty="Nothing saved yet. Open a title and choose Add to My List."
          >
            {saved.map((media) => (
              <MediaCard key={`saved-${media.id}`} media={media} />
            ))}
          </Shelf>
        ) : null}
      </div>

      <footer className={styles.footer}>
        <span>MEDIA HAVEN · Local library</span>
        <span>
          {totalTitles} titles · {roots.counts.episodes} episodes · no account · no
          telemetry
        </span>
        {catalogStale && (catalog.movies.length > 0 || catalog.tv.length > 0) ? (
          <span data-testid="catalog-stale-note">
            Catalog temporarily unavailable — showing last loaded titles
          </span>
        ) : null}
      </footer>
    </div>
  );
}
