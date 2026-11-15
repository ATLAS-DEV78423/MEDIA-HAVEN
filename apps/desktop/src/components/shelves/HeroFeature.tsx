import type { HeroFeature as HeroFeatureData } from "@/types/media";
import { backdropOf, formatRuntime } from "@/types/media";
import { Artwork } from "@/components/media/Artwork";
import styles from "./hero.module.css";

export interface HeroFeatureProps {
  readonly feature: HeroFeatureData;
  readonly onAction: (kind: HeroFeatureData["actions"][number]["kind"]) => void;
}

export function HeroFeature({ feature, onAction }: HeroFeatureProps) {
  const { media } = feature;
  const backdrop = backdropOf(media);
  const runtime = formatRuntime(media.runtimeMinutes);

  const metaParts = [
    media.year?.toString() ?? null,
    runtime,
    media.genres.slice(0, 3).join(", ") || null,
  ].filter((part): part is string => part !== null);

  return (
    <section className={styles.hero} aria-label="Featured">
      <Artwork
        artwork={backdrop}
        shape="landscape"
        alt=""
        className={styles.backdrop}
      />
      <div className={styles.scrim} />
      <div className={`${styles.content} m-enter`}>
        <div className={styles.kicker}>{media.kind === "series" ? "Series" : "Featured"}</div>
        <h1 className={styles.title}>{feature.headline}</h1>
        <div className={styles.meta}>
          {metaParts.map((part, index) => (
            <span key={part}>
              {index > 0 ? <span className={styles.dot}> · </span> : null}
              {part}
            </span>
          ))}
        </div>
        {media.synopsis ? (
          <p className={styles.synopsis}>{media.synopsis}</p>
        ) : null}
        <div className={styles.actions}>
          {feature.actions.map((action) => (
            <button
              key={action.kind}
              type="button"
              className={`${styles.button} ${
                action.kind === "resume" || action.kind === "play"
                  ? styles.primary
                  : styles.secondary
              }`}
              onClick={() => onAction(action.kind)}
            >
              {action.label}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}
