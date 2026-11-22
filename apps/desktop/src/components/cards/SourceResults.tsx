/* Shortlist panel for operator-chosen sources. Searching and choosing are
 * separate acts: nothing is downloaded or played automatically. Every status
 * line is fixed text — no magnet, path, address or key is rendered here. */

import { useState } from "react";

import { ActionButton } from "@/components/ui/Controls";
import { api, type SourceCandidate, type SourceQuery } from "@/services/api";
import { call } from "@/services/transport";
import styles from "./sourceResults.module.css";

export const SOURCE_STATUS = {
  empty: "No sources found.",
  failed: "Sources could not be found. Try again.",
  added: "Source sent to downloads and saved for this item.",
  addFailed: "The source could not be added. Try again.",
  mappingFailed: "Download started, but mapping could not be saved.",
} as const;

interface SourceResultsProps {
  mediaId: string;
  query: SourceQuery;
}

function readableSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Size unknown";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function SourceResults({ mediaId, query }: SourceResultsProps) {
  const [results, setResults] = useState<SourceCandidate[]>([]);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const findSources = async () => {
    setBusy(true);
    setStatus(null);
    setResults([]);
    try {
      const response = await api.sourceSearch(query);
      setResults(response.results);
      if (response.results.length === 0) setStatus(SOURCE_STATUS.empty);
    } catch {
      setStatus(SOURCE_STATUS.failed);
    } finally {
      setBusy(false);
    }
  };

  const choose = async (candidate: SourceCandidate) => {
    setBusy(true);
    setStatus(null);
    try {
      await call("download_add", { source: candidate.magnet });
      try {
        await api.sourceSet(mediaId, candidate.magnet);
        setStatus(SOURCE_STATUS.added);
      } catch {
        // The download is already running; only the mapping was lost.
        setStatus(SOURCE_STATUS.mappingFailed);
      }
    } catch {
      setStatus(SOURCE_STATUS.addFailed);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={styles.root} aria-label="Source search">
      <div className={styles.head}>
        <h2 className={styles.title}>Find sources</h2>
        <ActionButton variant="glass" onClick={() => void findSources()} disabled={busy}>
          {busy ? "Searching…" : "Find sources"}
        </ActionButton>
      </div>
      <p className={styles.hint}>
        Results are candidates only. Choosing one sends it to downloads and saves
        it for this title; nothing plays automatically.
      </p>
      {results.length > 0 ? (
        <ul className={styles.list}>
          {results.map((candidate) => (
            <li key={candidate.id} className={styles.row}>
              <span className={styles.copy}>
                <span className={styles.name}>{candidate.title}</span>
                <span className={styles.meta}>
                  {candidate.seeders} seeders · {readableSize(candidate.size)}
                </span>
              </span>
              <ActionButton
                variant="glass"
                onClick={() => void choose(candidate)}
                disabled={busy}
              >
                Choose
              </ActionButton>
            </li>
          ))}
        </ul>
      ) : null}
      {status ? (
        <p className={styles.status} role="status">
          {status}
        </p>
      ) : null}
    </section>
  );
}
