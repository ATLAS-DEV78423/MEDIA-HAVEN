import { Artwork } from "@/components/media/Artwork";
import styles from "./cards.module.css";

export interface CollectionCardProps {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string;
  readonly artworkUrl: string | null;
  readonly alt: string;
  readonly onOpen: (id: string) => void;
}

export function CollectionCard({
  id,
  title,
  subtitle,
  artworkUrl,
  alt,
  onOpen,
}: CollectionCardProps) {
  return (
    <button
      type="button"
      className={styles.card}
      data-shelf-item
      onClick={() => onOpen(id)}
      aria-label={`${title}, ${subtitle}`}
    >
      <Artwork
        artwork={artworkUrl ? { variant: "poster", url: artworkUrl, alt } : null}
        shape="poster"
        alt={alt}
      />
      <div className={styles.body}>
        <div className={styles.title}>{title}</div>
        <div className={styles.meta}>{subtitle}</div>
      </div>
    </button>
  );
}
