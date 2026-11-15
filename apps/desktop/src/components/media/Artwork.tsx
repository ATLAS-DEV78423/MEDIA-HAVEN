import { useState } from "react";
import type { ArtworkRef } from "@/types/media";
import styles from "./Artwork.module.css";

interface ArtworkProps {
  /** null means "no artwork for this variant" — a designed placeholder. */
  readonly artwork: ArtworkRef | null;
  /** Layout shape: "poster" (2:3) or "landscape" (16:9). */
  readonly shape?: "poster" | "landscape";
  /** Overrides the artwork's own alt text when the context needs more. */
  readonly alt?: string;
  readonly className?: string | undefined;
}

function PlaceholderGlyph() {
  return (
    <svg
      className={styles.glyph}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.25"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="2.5" y="5" width="19" height="14" rx="2.5" />
      <path d="M2.5 9.5h19" />
      <path d="M8 5v14" />
      <circle cx="14" cy="13.5" r="1.6" />
    </svg>
  );
}

export function Artwork({ artwork, shape = "poster", alt, className }: ArtworkProps) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  const frameClass = [
    styles.frame,
    shape === "poster" ? styles.poster : styles.landscape,
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");

  const showImage = artwork?.url != null && !failed;

  return (
    <div className={frameClass}>
      {showImage ? (
        <img
          className={`${styles.image} ${loaded ? styles.loaded : ""}`}
          src={artwork!.url!}
          alt={alt ?? artwork!.alt}
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setLoaded(true)}
          onError={() => setFailed(true)}
        />
      ) : (
        <div className={styles.placeholder}>
          <PlaceholderGlyph />
        </div>
      )}
      {!loaded && showImage ? <div className={styles.loading} /> : null}
    </div>
  );
}
