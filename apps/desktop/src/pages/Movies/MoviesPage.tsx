import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { CatalogShelf } from "@/components/catalog/CatalogShelf";
import styles from "@/components/catalog/catalog.module.css";
import { buttonClass, Segmented } from "@/components/ui/Controls";
import { api } from "@/services/api";
import { toMedia } from "@/services/adapt";
import type { CatalogScreen } from "@/services/types";
import { useAppState } from "@/state/AppState";

type Mode = "library" | "discover";
const modeOptions: { value: Mode; label: string }[] = [
  { value: "library", label: "My library" },
  { value: "discover", label: "Discover" },
];

export function MoviesPage() {
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
      .discover("movie")
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
        <CatalogShelf kind="movie" screen={catalog} />
      ) : (
        <BrowseView
          title="Movies"
          description="Films indexed from the folders this machine is allowed to read. No rentals, no streaming queue, no account."
          items={data.movies.map((summary) => ({
            ...toMedia(summary),
            imdbRating: summary.imdbRating,
            voteCount: summary.voteCount,
            ageRating: summary.ageRating,
          }))}
          emptyTitle="No films in the library yet"
          emptyBody="Add a folder that holds your films, then scan it. MEDIA HAVEN indexes what is already on the disk and leaves the files where they are."
          emptyAction={
            <Link to="/setup" className={buttonClass("solid")} data-focusable="true">
              Add a folder
            </Link>
          }
        />
      )}
    </>
  );
}
