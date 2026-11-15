import { useEffect, useRef, useState } from "react";
import type { SearchQuery, SearchResults } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useMediaActions } from "@/hooks/useMediaActions";
import { PageHeader, EmptyState, ErrorState } from "@/components/feedback/States";
import { MediaCard } from "@/components/cards/MediaCard";
import { SkeletonCard } from "@/components/cards/ProgressBar";
import styles from "@/components/feedback/page.module.css";

const DEBOUNCE_MS = 250;

export function SearchPage() {
  const actions = useMediaActions();
  const [text, setText] = useState("");
  const [debounced, setDebounced] = useState("");
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setDebounced(text), DEBOUNCE_MS);
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    };
  }, [text]);

  const query: SearchQuery = { text: debounced, kinds: [], genres: [] };

  const results = useCommand<SearchResults>(
    async () => {
      if (debounced.trim() === "") {
        return {
          query,
          items: [],
          total: 0,
          truncated: false,
        };
      }
      return (await getCommands()).library_search(query);
    },
    [debounced],
  );

  const hasQuery = debounced.trim() !== "";

  return (
    <>
      <PageHeader
        title="Search"
        subtitle="Searches your library on this machine."
      />

      <div className={styles.section}>
        <label htmlFor="search-input" className={styles.subheading}>
          Search
        </label>
        <input
          id="search-input"
          type="search"
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder="Title, person, genre…"
          aria-label="Search your library"
          style={{ width: "100%", maxWidth: "32rem", marginTop: "var(--space-3)" }}
        />
      </div>

      {results.status === "error" ? (
        <ErrorState error={results.error} onRetry={results.reload} />
      ) : null}

      {results.status === "loading" && hasQuery ? (
        <div className={styles.grid} data-shelf aria-busy="true" aria-label="Searching">
          {Array.from({ length: 12 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      ) : null}

      {results.status === "ready" && hasQuery ? (
        <>
          <div className={styles.section}>
            <p className={styles.subheading}>
              {results.data.total.toLocaleString()} result
              {results.data.total === 1 ? "" : "s"}
              {results.data.truncated ? ` · showing the first ${results.data.items.length}` : ""}
            </p>
          </div>
          {results.data.items.length === 0 ? (
            <EmptyState
              title="Nothing matched"
              body="Try a different title, person or genre. Search runs on this machine and works offline."
            />
          ) : (
            <div className={styles.grid} data-shelf>
              {results.data.items.map((media) => (
                <MediaCard key={media.id} media={media} onOpen={actions.openDetails} />
              ))}
            </div>
          )}
        </>
      ) : null}

      {!hasQuery ? (
        <EmptyState
          title="Search your library"
          body="Type a title, a person or a genre. Everything is found locally, with no network request."
        />
      ) : null}
    </>
  );
}
