import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { TmdbAttribution } from "@/components/cards/TmdbAttribution";
import { Icon } from "@/components/ui/Icon";
import { buttonClass } from "@/components/ui/Controls";
import { artFor, type Media } from "@/data/catalog";
import { useAppState } from "@/state/AppState";
import styles from "./hero.module.css";

const ROTATE_MS = 10000;

export function Hero({ slides }: { slides: Media[] }) {
  const { preferences } = useAppState();
  const [index, setIndex] = useState(0);
  const [paused, setPaused] = useState(false);

  const media = slides[index] ?? slides[0];

  useEffect(() => {
    if (!media || slides.length < 2 || paused || preferences.reduceMotion) {
      return;
    }
    const timer = window.setTimeout(() => {
      setIndex((current) => (current + 1) % slides.length);
    }, ROTATE_MS);
    return () => window.clearTimeout(timer);
  }, [index, paused, slides.length, media, preferences.reduceMotion]);

  if (!media) return null;

  const isSeries = media.kind === "series";
  const playLabel = isSeries ? "Episodes" : media.progress ? "Resume" : "Play";
  const href = isSeries ? `/details/${media.id}` : `/player/${media.id}`;
  const facts = [
    media.year ? String(media.year) : null,
    media.runtime || null,
    media.genres.slice(0, 2).join(" · "),
  ].filter((fact): fact is string => Boolean(fact));

  return (
    <section
      className={styles.hero}
      aria-label="Featured titles"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={() => setPaused(false)}
    >
      <div className={styles.stage} aria-hidden="true">
        {slides.map((slide, slideIndex) => (
          <img
            key={slide.id}
            src={slide.art.hero}
            alt=""
            className={`${styles.art} ${
              slideIndex === index ? styles.artActive : ""
            }`}
            draggable={false}
            decoding="async"
            fetchPriority={slideIndex === 0 ? "high" : "low"}
            onError={(event) => {
              const image = event.currentTarget;
              if (image.dataset.artFallback) return;
              image.dataset.artFallback = "true";
              image.src = artFor(slide.id, slide.title).hero;
            }}
          />
        ))}
        <div className={styles.scrimTop} />
        <div className={styles.scrimSide} />
        <div className={styles.scrimBottom} />
      </div>

      <div className={styles.content}>
        <div className={styles.copy} key={media.id}>
          <h1 className={styles.title}>{media.title}</h1>

          <div className={styles.meta}>
            {facts.map((fact) => (
              <span key={fact}>{fact}</span>
            ))}
          </div>

          <p className={styles.synopsis}>{media.synopsis}</p>

          <div className={styles.actions}>
            <Link to={href} className={buttonClass("solid")} data-focusable="true">
              <Icon name={isSeries ? "tv" : "play"} />
              <span>{playLabel}</span>
              {media.progress ? (
                <span className={styles.episode}>
                  {Math.round(media.progress * 100)}%
                </span>
              ) : null}
            </Link>
            <Link
              to={`/details/${media.id}`}
              className={buttonClass("glass")}
              data-focusable="true"
            >
              <Icon name="info" />
              <span>More info</span>
            </Link>
            <Link
              to={`/details/${media.id}`}
              className={styles.round}
              aria-label={`Open ${media.title}`}
              data-focusable="true"
            >
              <Icon name="chevronRight" />
            </Link>
          </div>
          <TmdbAttribution />
        </div>
      </div>

      {slides.length > 1 ? (
        <div className={styles.footer}>
          <div className={styles.dots} role="tablist" aria-label="Featured titles">
            {slides.map((slide, slideIndex) => (
              <button
                key={slide.id}
                type="button"
                role="tab"
                aria-selected={slideIndex === index}
                aria-label={`Show ${slide.title}`}
                className={`${styles.dot} ${
                  slideIndex === index ? styles.dotActive : ""
                }`}
                onClick={() => setIndex(slideIndex)}
                data-focusable="true"
              />
            ))}
          </div>
          <span className={styles.counter}>
            {String(index + 1).padStart(2, "0")} /{" "}
            {String(slides.length).padStart(2, "0")}
          </span>
        </div>
      ) : null}
    </section>
  );
}
