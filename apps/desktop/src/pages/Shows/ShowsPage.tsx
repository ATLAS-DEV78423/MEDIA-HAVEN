import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { CatalogShelf } from "@/components/catalog/CatalogShelf";
import styles from "@/components/catalog/catalog.module.css";
import { buttonClass, Segmented } from "@/components/ui/Controls";
import { api } from "@/services/api";
import { seriesToMedia } from "@/services/adapt";
import type { CatalogScreen } from "@/services/types";
import { useAppState } from "@/state/AppState";

type Mode = "library" | "discover";
const modeOptions: { value: Mode; label: string }[] = [
  { value: "library", label: "My library" },
  { value: "discover", label: "Discover" },
];

export function ShowsPage() {
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
      .discover("tv")
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
        <CatalogShelf kind="tv" screen={catalog} />
      ) : (
        <BrowseView
          title="TV Shows"
          description="Series, seasons and episodes from your own library. Episode lists are read from the backend, never guessed."
          items={data.shows.map(seriesToMedia)}
          emptyTitle="No series in the library yet"
          emptyBody="Add the folder that holds your series. Season and episode numbers are read from the file names, never guessed."
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
