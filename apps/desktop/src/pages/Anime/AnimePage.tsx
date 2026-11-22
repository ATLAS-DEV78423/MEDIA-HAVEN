import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { CatalogShelf } from "@/components/catalog/CatalogShelf";
import styles from "@/components/catalog/catalog.module.css";
import { buttonClass, Segmented } from "@/components/ui/Controls";
import { api } from "@/services/api";
import { seriesToMedia, toMedia } from "@/services/adapt";
import type { CatalogScreen } from "@/services/types";
import { useAppState } from "@/state/AppState";

type Mode = "library" | "discover";
const modeOptions: { value: Mode; label: string }[] = [
  { value: "library", label: "My library" },
  { value: "discover", label: "Discover" },
];

export function AnimePage() {
  const { data } = useAppState();
  // Local library stays the default; Discover is opt-in for this visit only.
  const [mode, setMode] = useState<Mode>("library");
  const [catalog, setCatalog] = useState<CatalogScreen | null>(null);

  const changeMode = (next: Mode) => {
    setMode(next);
    if (next === "discover") setCatalog(null);
  };

  useEffect(() => {
    if (mode !== "discover") return;
    let active = true;
    api
      .discover("anime")
      .then((result) => {
        if (active) setCatalog(result);
      })
      .catch(() => {
        // Failures degrade to an empty section, never a broken browse page.
        if (active) setCatalog({ available: true, entries: [] });
      });
    return () => {
      active = false;
    };
  }, [mode]);

  if (!data) return null;

  const anime = [
    ...data.movies.map(toMedia),
    ...data.shows.map(seriesToMedia),
  ].filter((media) =>
    media.genres.some((genre) => genre.toLowerCase() === "anime"),
  );

  return (
    <>
      <div className={styles.toggleRow}>
        <Segmented
          label="Browse mode"
          options={modeOptions}
          value={mode}
          onChange={changeMode}
        />
      </div>
      {mode === "discover" ? (
        <>
          <p className={styles.note}>
            Discover uses TMDB's Japanese-animation filter and may miss some
            anime. Your library's own Anime tag is unchanged in My library.
          </p>
          <CatalogShelf kind="anime" screen={catalog} />
        </>
      ) : (
        <BrowseView
          title="Anime"
          description="Anything tagged Anime in the library's own metadata. Tags come from the files and the backend, never from a catalogue service."
          items={anime}
          emptyTitle="No anime tagged yet"
          emptyBody="Titles appear here when their indexed genres include Anime. Add a folder or tag titles in the library metadata."
          emptyAction={
            <Link to="/movies" className={buttonClass("solid")} data-focusable="true">
              Browse everything
            </Link>
          }
        />
      )}
    </>
  );
}
