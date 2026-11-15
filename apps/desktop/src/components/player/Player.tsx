import { useEffect, useRef, useState } from "react";
import type { PlaybackSession } from "@/types/media";
import { formatTimestamp } from "@/types/media";
import { useOverlay } from "@/components/navigation/FocusManager";
import styles from "./player.module.css";

export interface PlayerProps {
  readonly session: PlaybackSession;
  readonly onClose: () => void;
  readonly nextUp?: { readonly title: string; readonly onPlay: () => void } | null;
}

const IDLE_MS = 3200;

function PlayIcon({ paused }: { readonly paused: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="24"
      height="24"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      {paused ? (
        <>
          <rect x="6" y="4" width="4.5" height="16" rx="1.4" />
          <rect x="13.5" y="4" width="4.5" height="16" rx="1.4" />
        </>
      ) : (
        <path d="M7 4.5 19.5 12 7 19.5z" />
      )}
    </svg>
  );
}

export function Player({ session, onClose, nextUp }: PlayerProps) {
  const [paused, setPaused] = useState(false);
  const [idle, setIdle] = useState(false);
  const [position, setPosition] = useState(session.startPositionSeconds);
  const idleTimer = useRef<number | null>(null);

  useOverlay(onClose);

  // The transport is mocked until the playback engine lands (P3-01). These
  // timers stand in for the engine's position events so the controls,
  // scrubbing and idle-fade behaviour can be built and tested now.
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (!paused) {
        setPosition((p) =>
          Math.min(session.durationSeconds, p + 1),
        );
      }
    }, 1000);
    return () => window.clearInterval(timer);
  }, [paused, session.durationSeconds]);

  useEffect(() => {
    const wake = () => {
      setIdle(false);
      if (idleTimer.current !== null) window.clearTimeout(idleTimer.current);
      idleTimer.current = window.setTimeout(() => setIdle(true), IDLE_MS);
    };
    wake();
    const container = document.getElementById("player-surface");
    container?.addEventListener("pointermove", wake);
    container?.addEventListener("keydown", wake);
    return () => {
      if (idleTimer.current !== null) window.clearTimeout(idleTimer.current);
      container?.removeEventListener("pointermove", wake);
      container?.removeEventListener("keydown", wake);
    };
  }, []);

  const ratio =
    session.durationSeconds > 0
      ? position / session.durationSeconds
      : 0;

  return (
    <div id="player-surface" className={styles.surface}>
      <div className={styles.surfacePlaceholder}>
        <p className={styles.surfacePlaceholderTitle}>
          {session.media.title}
        </p>
        <p className={styles.surfacePlaceholderBody}>
          Playback runs on the Windows host, where the GPU, the audio endpoint and
          the display live. The decoder and its controls are wired up in Phase 3.
        </p>
      </div>

      <div
        className={`${styles.overlay} ${idle ? styles.overlayIdle : ""}`}
        onPointerMove={() => setIdle(false)}
      >
        <div className={styles.titleBlock}>
          <p className={styles.title}>{session.media.title}</p>
          {session.media.synopsis ? (
            <p className={styles.subtitle}>{session.media.synopsis}</p>
          ) : null}
        </div>

        <div className={styles.controls}>
          <div
            className={styles.scrubber}
            role="slider"
            tabIndex={0}
            aria-label="Seek"
            aria-valuemin={0}
            aria-valuemax={session.durationSeconds}
            aria-valuenow={Math.round(position)}
            aria-valuetext={`${formatTimestamp(position)} of ${formatTimestamp(session.durationSeconds)}`}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight") {
                event.preventDefault();
                setPosition((p) =>
                  Math.min(session.durationSeconds, p + 15),
                );
              }
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                setPosition((p) => Math.max(0, p - 15));
              }
            }}
          >
            <div className={styles.scrubberFill} style={{ width: `${ratio * 100}%` }} />
            <div
              className={styles.scrubberKnob}
              style={{ left: `${ratio * 100}%` }}
            />
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className={`${styles.button} ${styles.playButton}`}
              onClick={() => setPaused((p) => !p)}
              aria-label={paused ? "Play" : "Pause"}
            >
              <PlayIcon paused={paused} />
            </button>

            <span className={styles.time}>
              {formatTimestamp(position)} / {formatTimestamp(session.durationSeconds)}
            </span>

            <span className={styles.spacer} />

            {session.subtitleTracks.length > 0 ? (
              <select
                className={styles.select}
                aria-label="Subtitles"
                defaultValue={
                  session.subtitleTracks.find((t) => t.selected)?.id ?? ""
                }
              >
                {session.subtitleTracks.map((track) => (
                  <option key={track.id} value={track.id}>
                    {track.label}
                  </option>
                ))}
              </select>
            ) : null}

            {session.audioTracks.length > 0 ? (
              <select
                className={styles.select}
                aria-label="Audio"
                defaultValue={
                  session.audioTracks.find((t) => t.selected)?.id ?? ""
                }
              >
                {session.audioTracks.map((track) => (
                  <option key={track.id} value={track.id}>
                    {track.label}
                  </option>
                ))}
              </select>
            ) : null}

            <button
              type="button"
              className={styles.button}
              onClick={onClose}
              aria-label="Close player"
            >
              <svg
                viewBox="0 0 24 24"
                width="22"
                height="22"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
                aria-hidden="true"
                focusable="false"
              >
                <path d="m6 6 12 12M18 6 6 18" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      {nextUp ? (
        <div className={styles.upNext} role="dialog" aria-label="Up next">
          <p className={styles.upNextTitle}>Up next</p>
          <p className={styles.upNextBody}>{nextUp.title}</p>
          <div className={styles.upNextActions}>
            <button
              type="button"
              className={`${styles.button} ${styles.playButton}`}
              onClick={nextUp.onPlay}
              aria-label={`Play ${nextUp.title}`}
            >
              <PlayIcon paused={false} />
            </button>
            <button
              type="button"
              className={styles.button}
              onClick={onClose}
              aria-label="Dismiss up next"
            >
              <svg
                viewBox="0 0 24 24"
                width="22"
                height="22"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
                aria-hidden="true"
                focusable="false"
              >
                <path d="m6 6 12 12M18 6 6 18" />
              </svg>
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
