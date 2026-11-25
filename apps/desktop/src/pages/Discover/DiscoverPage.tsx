import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { Chip, buttonClass } from "@/components/ui/Controls";
import { EmptyState } from "@/components/ui/Surfaces";
import { api, describeError, type CatalogItem, type CatalogResult } from "@/services/api";
import { normalizeTitle, similarity, suggestTitles } from "@/services/fuzzy";
import { CatalogCard } from "./CatalogCard";
import styles from "./discover.module.css";

/* Required TMDB credit and clear AniList source attribution. */
const TMDB_ATTRIBUTION =
  "This product uses the TMDB API but is not endorsed or certified by TMDB.";
const ANILIST_ATTRIBUTION = "Anime catalogue data provided by AniList.";

/** One typo-tolerant fallback query from prior catalog results, else a
 *  shorter version of the query. Returns null when there is nothing safer
 *  to try; the caller performs at most one retry. */
function alternateQuery(query: string, prior: CatalogItem[]): string | null {
  const suggestion = suggestTitles(
    query,
    prior.map((item) => ({ id: item.id, title: item.title, kind: item.kind })),
    1,
  )[0];
  if (suggestion) {
    const next = normalizeTitle(suggestion.title);
    if (next && next !== query) return next;
  }
  const words = query.split(" ").filter(Boolean);
  if (words.length > 1) {
    const shorter = words.slice(0, -1).join(" ");
    if (shorter && shorter !== query) return shorter;
  }
  return null;
}

export function DiscoverPage() {
  const navigate = useNavigate();
  const [kind, setKind] = useState<"movie" | "tv" | "anime">("movie");
  const [browse, setBrowse] = useState<CatalogItem[]>([]);
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [results, setResults] = useState<CatalogItem[] | null>(null);
  const [query, setQuery] = useState("");
  const [searchedFor, setSearchedFor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [setupRequired, setSetupRequired] = useState(false);
  const [addingId, setAddingId] = useState<string | null>(null);
  const [retryNonce, setRetryNonce] = useState(0);
  const [retrying, setRetrying] = useState(false);
  const searchSeq = useRef(0);

  useEffect(() => {
    let cancelled = false;
    const fetchBrowse = () =>
      kind === "anime" ? api.catalogAnime() : api.catalogBrowse(kind);
    const apply = (response: CatalogResult) => {
      if (cancelled) return;
      if (response.setup_required === true) {
        // No request was made: keep the last loaded list instead of clearing it.
        setSetupRequired(true);
        setBrowseError(null);
        return;
      }
      setBrowse(response.items ?? []);
      setSetupRequired(false);
      setBrowseError(null);
    };
    void (async () => {
      try {
        apply(await fetchBrowse());
      } catch {
        if (cancelled) return;
        // One bounded retry for transient throttling; setup_required never throws.
        await new Promise<void>((resolve) => setTimeout(resolve, 800));
        if (cancelled) return;
        try {
          apply(await fetchBrowse());
        } catch (failure) {
          if (!cancelled) setBrowseError(describeError(failure));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
          setRetrying(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [kind, retryNonce]);

  const search = useCallback(
    async (text: string) => {
      const seq = ++searchSeq.current;
      const alive = () => seq === searchSeq.current;
      if (!text) {
        setResults(null);
        setSearchedFor(null);
        return;
      }
      const normalised = normalizeTitle(text);
      if (!normalised) {
        setResults(null);
        setSearchedFor(null);
        return;
      }
      setLoading(true);
      setError(null);
      setBrowseError(null);
      setSearchedFor(null);
      let lastQuery = normalised;
      try {
        let response =
          kind === "anime"
            ? await api.catalogAnime(normalised)
            : await api.catalogSearch(normalised);
        let items = response.items ?? [];
        // Bounded: at most two extra queries, never recursion or a loop.
        if (kind !== "anime" && items.length === 0 && response.setup_required !== true) {
          const alternate = alternateQuery(normalised, [...browse, ...(results ?? [])]);
          if (alternate) {
            lastQuery = alternate;
            response = await api.catalogSearch(alternate);
            items = response.items ?? [];
          }
          // A single-word typo has no word to drop, so try a shorter prefix.
          if (items.length === 0 && normalised.length >= 5) {
            const prefix = normalised.slice(0, -2);
            if (prefix && prefix !== lastQuery) {
              lastQuery = prefix;
              response = await api.catalogSearch(prefix);
              const merged = new Map<string, CatalogItem>();
              for (const item of [...items, ...(response.items ?? [])]) {
                if (!merged.has(item.id)) merged.set(item.id, item);
              }
              items = [...merged.values()].sort(
                (a, b) =>
                  similarity(normalizeTitle(b.title), normalised) -
                    similarity(normalizeTitle(a.title), normalised) ||
                  a.title.localeCompare(b.title),
              );
            }
          }
        }
        if (!alive()) return;
        setResults(items);
        setSearchedFor(lastQuery !== text.trim() ? lastQuery : null);
        setSetupRequired(response.setup_required === true);
      } catch (failure) {
        if (!alive()) return;
        setResults([]);
        setError(describeError(failure));
      } finally {
        if (alive()) setLoading(false);
      }
    },
    [browse, kind, results],
  );

  const addWanted = async (item: CatalogItem) => {
    setAddingId(item.id);
    setError(null);
    try {
      const { mediaId } = await api.catalogWantedAdd(
        item.title,
        item.year,
        item.kind,
      );
      navigate(`/details/${mediaId}`);
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setAddingId(null);
    }
  };

  const items = results ?? browse;
  const shownError = error ?? (browse.length === 0 ? browseError : null);
  const sectionTitle = results
    ? "Search results"
    : kind === "movie"
      ? "Popular movies"
      : kind === "tv"
        ? "Popular TV"
        : "Popular anime";

  const retryBrowse = () => {
    setRetrying(true);
    setRetryNonce((nonce) => nonce + 1);
  };

  const selectKind = (next: "movie" | "tv" | "anime") => {
    setKind(next);
    setQuery("");
    setResults(null);
    setSearchedFor(null);
    setLoading(true);
    setError(null);
    setBrowseError(null);
    setSetupRequired(false);
  };

  return (
    <div className={`page ${styles.page}`}>
      <header className={styles.head}>
        <h1 className={styles.title}>Discover</h1>
        <p className={styles.sub}>
          Browse an optional online catalog. Picking a title creates a local
          wanted item — nothing is downloaded until you choose a source on the
          details screen.
        </p>
        <div className={styles.tabs} role="group" aria-label="Catalog type">
          <Chip active={kind === "movie"} onClick={() => selectKind("movie")}>
            Movies
          </Chip>
          <Chip active={kind === "tv"} onClick={() => selectKind("tv")}>
            TV
          </Chip>
          <Chip active={kind === "anime"} onClick={() => selectKind("anime")}>
            Anime
          </Chip>
        </div>
        <form
          className={styles.search}
          onSubmit={(event) => {
            event.preventDefault();
            void search(query.trim());
          }}
        >
          <input
            type="search"
            aria-label="Search the catalog"
            placeholder="Search titles"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              if (!event.target.value) {
                setResults(null);
                setSearchedFor(null);
                setError(null);
                setBrowseError(null);
                setSetupRequired(false);
              }
            }}
          />
          <button type="submit" className={buttonClass("solid")} data-focusable="true">
            Search
          </button>
        </form>
        {searchedFor ? (
          <p className={styles.count} data-testid="normalized-query">
            Showing results for {searchedFor}
          </p>
        ) : null}
      </header>

      {setupRequired ? (
        <EmptyState
          title="The catalog is not set up"
          body="An operator must enable the TMDB opt-in and add an API key. No request was made."
        />
      ) : shownError ? (
        <EmptyState
          title="The catalog is unavailable"
          body={shownError}
          action={
            <button
              type="button"
              className={buttonClass("solid")}
              data-focusable="true"
              disabled={retrying}
              onClick={retryBrowse}
            >
              Retry
            </button>
          }
        />
      ) : loading ? (
        <EmptyState
          title={results ? "Searching the catalog…" : "Loading the catalog…"}
          body="The host is answering. Browsing never starts a download."
        />
      ) : items.length === 0 ? (
        <EmptyState
          title={results ? "No matching titles" : "Nothing to show"}
          body={
            results
              ? "Try another spelling, or browse the catalog below."
              : "The catalog returned no titles for this category."
          }
        />
      ) : (
        <>
          {browseError && !results ? (
            <p className={styles.count} data-testid="catalog-stale-note">
              Catalog temporarily unavailable — showing last loaded titles
            </p>
          ) : null}
          <div className={styles.listHead}>
            <h2 className={styles.sectionTitle}>{sectionTitle}</h2>
            <span className={styles.count}>
              {items.length} {items.length === 1 ? "title" : "titles"}
            </span>
          </div>
          <div className={styles.grid}>
            {items.map((item) => (
              <CatalogCard
                key={item.id}
                item={item}
                adding={addingId === item.id}
                onPlay={() => void addWanted(item)}
              />
            ))}
          </div>
          <p className={styles.attribution}>
            {kind === "anime" ? ANILIST_ATTRIBUTION : TMDB_ATTRIBUTION}
          </p>
        </>
      )}
    </div>
  );
}
