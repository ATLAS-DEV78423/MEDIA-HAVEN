import { Link } from "react-router-dom";

import { Hero } from "@/components/hero/Hero";
import { MediaCard } from "@/components/cards/MediaCard";
import { Shelf } from "@/components/shelf/Shelf";
import { buttonClass } from "@/components/ui/Controls";
import { EmptyState } from "@/components/ui/Surfaces";
import { toMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";
import styles from "./home.module.css";

const watchedLabel = (progress: number | undefined): string | undefined => {
  if (progress === undefined) return undefined;
  return `${Math.round(progress * 100)}% watched`;
};

export function HomePage() {
  const { data, activeProfile } = useAppState();
  if (!data) return null;

  const { home, watchlist, roots } = data;
  const heroMedia = home.hero
    ? { ...toMedia(home.hero.media), title: home.hero.headline || home.hero.media.title }
    : null;

  const shelves = home.shelves
    .map((shelf) => ({ id: shelf.id, title: shelf.title, items: shelf.items.map(toMedia) }))
    .filter((shelf) => shelf.items.length > 0);

  const saved = watchlist.entries.map((entry) => toMedia(entry.media));
  const totalTitles = roots.counts.movies + roots.counts.shows;
  const empty = !heroMedia && shelves.length === 0 && saved.length === 0;

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
      </footer>
    </div>
  );
}
