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
import {
  attachPlayerSurface,
  isTauriHost,
  listenPlayerState,
  playerControl,
} from "@/services/player";
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
  // Once a real engine event arrives the 4x dev clock is retired for good.
  const [engineSeen, setEngineSeen] = useState(false);
  const [chrome, setChrome] = useState(true);
  const [muted, setMuted] = useState(false);

  const elapsedRef = useRef(0);
  useEffect(() => {
    elapsedRef.current = elapsed;
  }, [elapsed]);

  const rootRef = useRef<HTMLDivElement>(null);
  const topRef = useRef<HTMLDivElement>(null);
  const controlsRef = useRef<HTMLDivElement>(null);

  const totalSeconds = Math.max(
    60,
    session && session.durationSeconds > 0 ? session.durationSeconds : 5400,
  );

  const reportSurface = useCallback(async () => {
    if (!isTauriHost()) return;
    const top = topRef.current?.getBoundingClientRect();
    const controls = controlsRef.current?.getBoundingClientRect();
    if (!top || !controls) {
      // shortcut: 1x1 placeholder creates the HWND before mpv spawns; the session effect sets the real rect.
      await attachPlayerSurface({ x: 0, y: 0, width: 1, height: 1 });
      return;
    }
    await attachPlayerSurface({
      x: 0,
      y: top.bottom,
      width: window.innerWidth,
      height: Math.max(1, controls.top - top.bottom),
    });
  }, []);

  const start = useCallback(async () => {
    if (!mediaId) return;
    setLoadError(null);
    await reportSurface().catch(() => {});
    try {
      const opened = await api.playbackStart(mediaId);
      setSession(opened);
      setElapsed(opened.startPositionSeconds);
      setPlaying(true);
    } catch (error) {
      setLoadError(describeError(error));
    }
  }, [mediaId, reportSurface]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void start();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [start]);

  useEffect(
    () =>
      listenPlayerState((event) => {
        setEngineSeen(true);
        setElapsed(event.timePos);
        setPlaying(!event.pause);
      }),
    [],
  );

  useEffect(() => {
    if (!session || !isTauriHost()) return;
    const root = rootRef.current;
    if (!root) return;
    const onChange = () => {
      void reportSurface().catch(() => {});
    };
    const observer = new ResizeObserver(onChange);
    observer.observe(root);
    onChange();
    return () => observer.disconnect();
  }, [session, reportSurface]);

  const save = useCallback(async () => {
    if (!mediaId || !session) return;
    await saveProgress(mediaId, elapsedRef.current, totalSeconds);
  }, [mediaId, session, totalSeconds, saveProgress]);

  const active = Boolean(session) && playing && elapsed < totalSeconds;

  // Dev-only fallback: no engine stream has been seen (browser without Tauri).
  useEffect(() => {
    if (!active || engineSeen) return;
    const timer = window.setInterval(() => {
      setElapsed((current) => Math.min(totalSeconds, current + SIMULATION_RATE));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [active, engineSeen, totalSeconds]);

  useEffect(() => {
    if (!session) return;
    const timer = window.setInterval(() => {
      if (active) void save();
    }, SAVE_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [active, save, session]);

  // Stop only on real unmount: `save` changes identity when the session
  // arrives, so a `save`-dependent cleanup would kill mpv right after start.
  useEffect(() => {
    return () => {
      void playerControl("stop").catch(() => {});
    };
  }, []);

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

  const seekTo = useCallback(
    (position: number) => {
      const next = Math.min(totalSeconds, Math.max(0, position));
      setElapsed(next);
      void playerControl("seek", next).catch(() => {});
      wake();
    },
    [totalSeconds, wake],
  );

  const seekBy = useCallback(
    (deltaSeconds: number) => seekTo(elapsedRef.current + deltaSeconds),
    [seekTo],
  );

  const togglePlay = useCallback(() => {
    const next = !playing;
    setPlaying(next);
    void playerControl(next ? "play" : "pause").catch(() => {});
    wake();
  }, [playing, wake]);

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
          togglePlay();
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
  }, [navigate, seekBy, togglePlay, wake]);

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
    <div ref={rootRef} className={styles.player}>
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

      <div
        ref={topRef}
        className={`${styles.top} ${chromeShown ? styles.shown : styles.hidden}`}
      >
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
            onClick={() => seekTo(95)}
            data-focusable="true"
          >
            <Icon name="skipForward" />
            Skip intro
          </button>
        ) : null}
      </div>

      <div
        ref={controlsRef}
        className={`${styles.controls} ${chromeShown ? styles.shown : styles.hidden}`}
      >
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
              seekTo((Number(event.target.value) / 1000) * totalSeconds);
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
              onClick={() => togglePlay()}
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
                const next = !muted;
                setMuted(next);
                void playerControl("volume", next ? 0 : 100).catch(() => {});
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
