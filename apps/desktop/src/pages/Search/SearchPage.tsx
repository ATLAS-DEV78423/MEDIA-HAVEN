import { useEffect, useRef, useState } from "react";

import { MediaCard } from "@/components/cards/MediaCard";
import { Shelf } from "@/components/shelf/Shelf";
import { CategoryTile } from "@/components/ui/CategoryTile";
import { Chip } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import { EmptyState } from "@/components/ui/Surfaces";
import { api, describeError } from "@/services/api";
import { toMedia } from "@/services/adapt";
import type { MediaSummary } from "@/services/types";
import { useAppState } from "@/state/AppState";
import styles from "./search.module.css";

const totalLabel = (summary: MediaSummary): string => {
  if (summary.kind === "series") return "Series";
  return [summary.year ? String(summary.year) : null, summary.runtimeMinutes ? `${summary.runtimeMinutes} min` : null]
    .filter((part): part is string => Boolean(part))
    .join(" · ");
};

export function SearchPage() {
  const { data, recentSearches, rememberSearch } = useAppState();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MediaSummary[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target;
      const typing =
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);
      if (event.key === "/" && !typing) {
        event.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const trimmed = query.trim();

  useEffect(() => {
    if (!trimmed) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setSearching(true);
      setError(null);
      api
        .search(trimmed)
        .then((response) => {
          if (!cancelled) setResults(response.items);
        })
        .catch((failure: unknown) => {
          if (!cancelled) {
            setResults([]);
            setError(describeError(failure));
          }
        })
        .finally(() => {
          if (!cancelled) setSearching(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trimmed]);

  if (!data) return null;

  const recentlyAdded = (
    data.home.shelves.find((shelf) => shelf.id === "recently-added-movies")?.items ??
    data.movies.slice(0, 6)
  ).slice(0, 6);

  const onChange = (value: string) => {
    setQuery(value);
    if (!value.trim()) {
      setResults(null);
      setError(null);
      setSearching(false);
    }
  };

  const submit = () => {
    if (trimmed) rememberSearch(trimmed);
  };

  return (
    <div className={`page ${styles.page}`}>
      <header className={styles.head}>
        <h1 className={styles.title}>Search</h1>
        <p className={styles.sub}>
          Films and series from the local database. Nothing is queried online.
        </p>

        <div className={styles.field}>
          <Icon name="search" className={styles.fieldIcon} />
          <input
            ref={inputRef}
            type="search"
            value={query}
            placeholder="Search titles, genres, years"
            aria-label="Search your library"
            autoFocus
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submit();
              if (event.key === "Escape") {
                onChange("");
                inputRef.current?.blur();
              }
            }}
          />
          {query ? (
            <button
              type="button"
              className={styles.clear}
              aria-label="Clear search"
              onClick={() => {
                onChange("");
                inputRef.current?.focus();
              }}
            >
              <Icon name="close" />
            </button>
          ) : (
            <kbd className={styles.kbd}>/</kbd>
          )}
        </div>

        {recentSearches.length > 0 ? (
          <div className={styles.recent}>
            <span className={styles.recentLabel}>Recent</span>
            {recentSearches.map((term) => (
              <Chip key={term} onClick={() => onChange(term)}>
                {term}
              </Chip>
            ))}
          </div>
        ) : null}
      </header>

      {trimmed ? (
        error ? (
          <EmptyState
            title="Search is unavailable"
            body={error}
            action={
              <Chip active onClick={() => onChange("")}>
                Clear search
              </Chip>
            }
          />
        ) : searching && !results ? (
          <EmptyState
            title="Searching the library…"
            body="The backend is answering. Results come from the local database."
          />
        ) : results && results.length > 0 ? (
          <>
            <div className={styles.resultsHead}>
              <h2 className={styles.resultsTitle}>Results</h2>
              <span className={styles.resultsCount}>
                {results.length} {results.length === 1 ? "result" : "results"}
              </span>
            </div>
            <div className={styles.grid}>
              {results.map((summary) => (
                <MediaCard
                  key={summary.id}
                  media={toMedia(summary)}
                  fill
                  label={totalLabel(summary)}
                />
              ))}
            </div>
          </>
        ) : (
          <EmptyState
            title={`Nothing matches “${trimmed}”`}
            body="Search looks at titles, genres and years in the local database. Check the spelling, or browse a category instead."
            action={
              <Chip active onClick={() => onChange("")}>
                Clear search
              </Chip>
            }
          />
        )
      ) : (
        <>
          <div className={styles.categories}>
            <CategoryTile
              to="/movies"
              label="Movies"
              count={`${data.roots.counts.movies} titles`}
              icon="film"
            />
            <CategoryTile
              to="/shows"
              label="TV Shows"
              count={`${data.roots.counts.shows} series`}
              icon="tv"
            />
            <CategoryTile
              to="/library"
              label="Library"
              count={`${data.roots.counts.episodes} episodes`}
              icon="grid"
            />
            <CategoryTile
              to="/downloads"
              label="Downloads"
              count={data.downloads.enabled ? "Enabled" : "Off"}
              icon="download"
            />
          </div>

          {recentlyAdded.length > 0 ? (
            <div className={styles.recentlyAdded}>
              <Shelf title="Recently Added">
                {recentlyAdded.map((summary) => (
                  <MediaCard key={summary.id} media={toMedia(summary)} />
                ))}
              </Shelf>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
