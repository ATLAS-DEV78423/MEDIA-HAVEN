import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { Icon } from "@/components/ui/Icon";
import { artFor } from "@/data/catalog";
import { api, describeError } from "@/services/api";
import type { PlaybackSession } from "@/services/types";
import { useAppState } from "@/state/AppState";
import styles from "./player.module.css";

const SIMULATION_RATE = 4;
const IDLE_MS = 3600;
const SAVE_EVERY_MS = 5000;

const pad = (value: number): string => String(value).padStart(2, "0");

const clock = (seconds: number): string => {
  const whole = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = whole % 60;
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(rest)}` : `${minutes}:${pad(rest)}`;
};

export function PlayerPage() {
  const { mediaId } = useParams();
  const navigate = useNavigate();
  const { saveProgress, refresh, preferences } = useAppState();

  const [session, setSession] = useState<PlaybackSession | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [chrome, setChrome] = useState(true);
  const [muted, setMuted] = useState(false);

  const elapsedRef = useRef(0);
  useEffect(() => {
    elapsedRef.current = elapsed;
  }, [elapsed]);

  const totalSeconds = Math.max(
    60,
    session && session.durationSeconds > 0 ? session.durationSeconds : 5400,
  );

  const start = useCallback(async () => {
    if (!mediaId) return;
    setLoadError(null);
    try {
      const opened = await api.playbackStart(mediaId);
      setSession(opened);
      setElapsed(opened.startPositionSeconds);
      setPlaying(true);
    } catch (error) {
      setLoadError(describeError(error));
    }
  }, [mediaId]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void start();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [start]);

  const save = useCallback(async () => {
    if (!mediaId || !session) return;
    await saveProgress(mediaId, elapsedRef.current, totalSeconds);
  }, [mediaId, session, totalSeconds, saveProgress]);

  const active = Boolean(session) && playing && elapsed < totalSeconds;

  useEffect(() => {
    if (!active) return;
    const timer = window.setInterval(() => {
      setElapsed((current) => Math.min(totalSeconds, current + SIMULATION_RATE));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [active, totalSeconds]);

  useEffect(() => {
    if (!session) return;
    const timer = window.setInterval(() => {
      if (active) void save();
    }, SAVE_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [active, save, session]);

  useEffect(() => {
    return () => {
      void (async () => {
        await save();
        await refresh();
      })();
    };
  }, [save, refresh]);

  useEffect(() => {
    if (!active) return;
    const timer = window.setTimeout(() => setChrome(false), IDLE_MS);
    return () => window.clearTimeout(timer);
  }, [active, chrome]);

  const chromeShown = chrome || !active;

  const wake = useCallback(() => {
    setChrome(true);
  }, []);

  const seekBy = useCallback(
    (deltaSeconds: number) => {
      setElapsed((current) =>
        Math.min(totalSeconds, Math.max(0, current + deltaSeconds)),
      );
      wake();
    },
    [totalSeconds, wake],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA")
      ) {
        return;
      }
      switch (event.key) {
        case " ":
          event.preventDefault();
          setPlaying((current) => !current);
          wake();
          break;
        case "ArrowLeft":
          event.preventDefault();
          seekBy(-15);
          break;
        case "ArrowRight":
          event.preventDefault();
          seekBy(15);
          break;
        case "Escape":
          event.preventDefault();
          navigate(-1);
          break;
        case "f":
          if (document.fullscreenElement) {
            void document.exitFullscreen();
          } else {
            void document.documentElement.requestFullscreen().catch(() => {});
          }
          break;
        default:
          wake();
      }
    };
    const onMove = () => wake();

    window.addEventListener("keydown", onKey);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("pointerdown", onMove);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("pointerdown", onMove);
    };
  }, [navigate, seekBy, wake]);

  useEffect(() => {
    document.documentElement.dataset.player = "active";
    return () => {
      delete document.documentElement.dataset.player;
    };
  }, []);

  const media = session?.media;
  const art = media ? artFor(media.id, media.title) : null;
  const ratio = Math.min(1, elapsed / totalSeconds);
  const remaining = Math.max(0, totalSeconds - elapsed);
  const showSkipIntro =
    media?.kind === "episode" && preferences.skipIntro && ratio < 0.12;
  const facts = media
    ? [
        media.kind === "episode" ? "Episode" : media.year ? String(media.year) : null,
        media.kind === "episode" ? null : media.genres.slice(0, 2).join(", "),
      ].filter((fact): fact is string => Boolean(fact))
    : [];

  if (!session) {
    return (
      <div className={styles.player}>
        <div className={styles.top}>
          <button
            type="button"
            className={styles.round}
            onClick={() => navigate(-1)}
            aria-label="Leave playback"
            data-focusable="true"
          >
            <Icon name="chevronLeft" />
          </button>
        </div>
        <div className={styles.interstitial}>
          <h1 className={styles.interstitialTitle}>
            {loadError ? "Playback could not start" : "Opening playback…"}
          </h1>
          <p className={styles.interstitialBody}>
            {loadError ??
              "The backend is preparing a session for this title."}
          </p>
          {loadError ? (
            <button
              type="button"
              className={styles.pill}
              onClick={() => void start()}
              data-focusable="true"
            >
              <Icon name="play" />
              Retry
            </button>
          ) : null}
          <Link to="/" className={styles.interstitialLink} data-focusable="true">
            Back to home
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.player}>
      <div className={styles.stage}>
        {art ? (
          <img
            className={`${styles.art} ${active ? styles.artPlaying : ""}`}
            src={art.hero}
            alt=""
            draggable={false}
          />
        ) : null}
        <div className={styles.vignette} aria-hidden="true" />
      </div>

      <div className={`${styles.top} ${chromeShown ? styles.shown : styles.hidden}`}>
        <button
          type="button"
          className={styles.round}
          onClick={() => navigate(-1)}
          aria-label="Leave playback"
          data-focusable="true"
        >
          <Icon name="chevronLeft" />
        </button>
        <div className={styles.now}>
          <span className={styles.nowTitle}>{media?.title}</span>
          <span className={styles.nowSub}>{facts.join(" · ")}</span>
        </div>
        <span className={styles.preview}>Playback preview</span>
      </div>

      <div className={`${styles.pills} ${chromeShown ? styles.shown : styles.hidden}`}>
        {showSkipIntro ? (
          <button
            type="button"
            className={styles.pill}
            onClick={() => {
              setElapsed(Math.min(totalSeconds, 95));
              wake();
            }}
            data-focusable="true"
          >
            <Icon name="skipForward" />
            Skip intro
          </button>
        ) : null}
      </div>

      <div className={`${styles.controls} ${chromeShown ? styles.shown : styles.hidden}`}>
        <div className={styles.progressRow}>
          <span className={styles.time}>{clock(elapsed)}</span>
          <input
            className={styles.range}
            type="range"
            min={0}
            max={1000}
            step={1}
            value={Math.round(ratio * 1000)}
            aria-label="Seek"
            style={{ "--seek": `${ratio * 100}%` } as CSSProperties}
            onChange={(event) => {
              const next = Number(event.target.value) / 1000;
              setElapsed(next * totalSeconds);
              wake();
            }}
            onMouseUp={() => void save()}
            onTouchEnd={() => void save()}
          />
          <span className={styles.time}>-{clock(remaining)}</span>
        </div>

        <div className={styles.transport}>
          <div className={styles.side}>
            <button
              type="button"
              className={styles.round}
              onClick={() => seekBy(-15)}
              aria-label="Back 15 seconds"
              data-focusable="true"
            >
              <Icon name="skipBack" />
            </button>
            <button
              type="button"
              className={`${styles.round} ${styles.play}`}
              onClick={() => {
                setPlaying((current) => !current);
                wake();
              }}
              aria-label={active ? "Pause" : "Play"}
              data-focusable="true"
            >
              <Icon name={active ? "pause" : "play"} />
            </button>
            <button
              type="button"
              className={styles.round}
              onClick={() => seekBy(15)}
              aria-label="Forward 15 seconds"
              data-focusable="true"
            >
              <Icon name="skipForward" />
            </button>
          </div>

          <div className={styles.sideRight}>
            <button
              type="button"
              className={styles.round}
              onClick={() => {
                setMuted((current) => !current);
                wake();
              }}
              aria-label={muted ? "Unmute" : "Mute"}
              aria-pressed={muted}
              data-focusable="true"
            >
              <Icon name={muted ? "mute" : "volume"} />
            </button>
            <button
              type="button"
              className={styles.round}
              onClick={() => {
                if (document.fullscreenElement) {
                  void document.exitFullscreen();
                } else {
                  void document
                    .documentElement
                    .requestFullscreen()
                    .catch(() => {});
                }
                wake();
              }}
              aria-label="Full screen"
              data-focusable="true"
            >
              <Icon name="expand" />
            </button>
          </div>
        </div>

        <p className={styles.hint}>
          Progress is saved to the local database · Space pause · ← → seek · Esc leave
        </p>
      </div>
    </div>
  );
}
