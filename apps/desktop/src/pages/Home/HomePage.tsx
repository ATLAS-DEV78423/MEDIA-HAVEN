import type { HomeScreen } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useMediaActions } from "@/hooks/useMediaActions";
import { HeroFeature } from "@/components/shelves/HeroFeature";
import { MediaShelf } from "@/components/shelves/MediaShelf";
import { MediaCard } from "@/components/cards/MediaCard";
import { SkeletonCard } from "@/components/cards/ProgressBar";
import { EmptyState, ErrorState } from "@/components/feedback/States";

function LibraryGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="3" y="4.5" width="7" height="15" rx="1.6" />
      <rect x="14" y="4.5" width="7" height="15" rx="1.6" />
    </svg>
  );
}

export function HomePage() {
  const actions = useMediaActions();

  const home = useCommand<HomeScreen>(
    async () => (await getCommands()).library_home(),
    [],
  );

  if (home.status === "error") {
    return <ErrorState error={home.error} onRetry={home.reload} />;
  }

  if (home.status === "loading") {
    return (
      <div aria-busy="true" aria-label="Loading your library">
        <div style={{ padding: "var(--space-9) var(--page-gutter)" }}>
          <div
            style={{
              height: "var(--text-hero)",
              width: "18rem",
              borderRadius: "var(--radius-pill)",
              backgroundColor: "var(--skeleton-base)",
            }}
          />
        </div>
        <MediaShelf title="Continue Watching">
          {Array.from({ length: 8 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </MediaShelf>
        <MediaShelf title="Recently Added">
          {Array.from({ length: 8 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </MediaShelf>
      </div>
    );
  }

  const shelves = home.data.shelves.filter((shelf) => shelf.items.length > 0);

  if (shelves.length === 0 && !home.data.hero) {
    return (
      <EmptyState
        title="Your library is empty"
        body="Add a folder that holds your films and series. MEDIA_HAVEN reads them in place and never moves or changes them."
        glyph={<LibraryGlyph />}
      />
    );
  }

  return (
    <>
      {home.data.hero ? (
        <HeroFeature
          feature={home.data.hero}
          onAction={(kind) => {
            switch (kind) {
              case "play":
              case "resume":
                actions.play(home.data.hero!.media);
                break;
              case "details":
                actions.openDetails(home.data.hero!.media);
                break;
              case "queue":
                actions.addToQueue(home.data.hero!.media);
                break;
            }
          }}
        />
      ) : null}

      {shelves.map((shelf) => (
        <MediaShelf key={shelf.id} title={shelf.title} count={shelf.items.length}>
          {shelf.items.map((media) => (
            <MediaCard
              key={`${shelf.id}-${media.id}`}
              media={media}
              onOpen={actions.openDetails}
            />
          ))}
        </MediaShelf>
      ))}
    </>
  );
}
