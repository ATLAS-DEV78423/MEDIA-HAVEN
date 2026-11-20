import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import { buttonClass } from "@/components/ui/Controls";
import type { AcquisitionRow } from "@/services/types";
import { call } from "@/services/transport";
import styles from "./downloads.module.css";

interface AddSourceDialogProps {
  onClose: () => void;
  onAdded: (row: AcquisitionRow) => void;
}

/** Host-side `download_add` is the only path from this dialog to qBittorrent. */
export function AddSourceDialog({ onClose, onAdded }: AddSourceDialogProps) {
  const [source, setSource] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const row = await call<AcquisitionRow>("download_add", { source: source.trim() });
      onAdded(row);
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "The source was rejected.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className={styles.overlay}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-source-title"
      >
        <h2 id="add-source-title" className={styles.listTitle}>
          Add a source
        </h2>
        <p className={styles.itemMeta}>
          Paste a magnet link or a .torrent file name you are authorized to
          download. Nothing is searched or discovered for you.
        </p>
        <form onSubmit={(event) => void submit(event)} className={styles.dialogForm}>
          <label className={styles.fieldLabel} htmlFor="add-source-input">
            Magnet link or .torrent file name
          </label>
          <input
            id="add-source-input"
            ref={inputRef}
            className={styles.input}
            value={source}
            onChange={(event) => setSource(event.target.value)}
            placeholder="magnet:?xt=urn:btih:"
            autoComplete="off"
            aria-invalid={error ? true : undefined}
          />
          {error ? (
            <p className={styles.error} role="alert">
              {error}
            </p>
          ) : null}
          <div className={styles.dialogActions}>
            <button
              type="button"
              className={buttonClass("glass")}
              data-focusable="true"
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              className={buttonClass("solid")}
              data-focusable="true"
              disabled={busy}
            >
              {busy ? "Adding…" : "Add"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
