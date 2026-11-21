import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { Segmented, Toggle, buttonClass } from "@/components/ui/Controls";
import { FieldRow, PageHeader, Panel } from "@/components/ui/Surfaces";
import { call } from "@/services/transport";
import type { RuntimeStatus } from "@/services/types";
import type { SubtitleSize } from "@/state/AppState";
import { useAppState } from "@/state/AppState";
import styles from "./settings.module.css";

const subtitleOptions: { value: SubtitleSize; label: string }[] = [
  { value: "small", label: "Small" },
  { value: "medium", label: "Medium" },
  { value: "large", label: "Large" },
];

const runtimeLabel = (status: RuntimeStatus): string => {
  if (status.state === "unhealthy") return `Unhealthy · ${status.message}`;
  switch (status.state) {
    case "ready":
      return "Ready";
    case "starting":
      return "Starting";
    case "stopped":
      return "Stopped";
    case "absent":
      return "Not installed";
    default:
      return "Unknown";
  }
};

export function SettingsPage() {
  const { data, activeProfile, preferences, updatePreference } = useAppState();
  const [downloadRoot, setDownloadRoot] = useState("");
  const [rootDraft, setRootDraft] = useState("");
  const [rootError, setRootError] = useState<string | null>(null);
  const [rootSaved, setRootSaved] = useState(false);

  useEffect(() => {
    let active = true;
    void call<string>("download_root_get", {})
      .then((value) => {
        if (!active) return;
        setDownloadRoot(value);
        setRootDraft(value);
      })
      .catch(() => {
        if (active) setRootError("The acquisition folder could not be read.");
      });
    return () => {
      active = false;
    };
  }, []);

  const saveRoot = async () => {
    try {
      const value = await call<string>("download_root_set", { path: rootDraft });
      setDownloadRoot(value);
      setRootDraft(value);
      setRootError(null);
      setRootSaved(true);
    } catch {
      setRootError("Enter an absolute folder path. The directory could not be set.");
      setRootSaved(false);
    }
  };

  if (!data) return null;

  const { settings, roots } = data;
  const totalItems =
    roots.counts.movies + roots.counts.shows + roots.counts.episodes;

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title="Settings"
        description="Playback, appearance and the folders this appliance is allowed to read. Everything is stored in the local database."
      />

      <div className={styles.body}>
        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Playback</h2>
          <Panel>
            <FieldRow
              label="Autoplay next episode"
              hint="Stored by the backend and applied by the player."
            >
              <Toggle
                label="Autoplay next episode"
                checked={preferences.autoplayNext}
                onChange={(next) => void updatePreference("autoplayNext", next)}
              />
            </FieldRow>
            <FieldRow
              label="Offer Skip Intro"
              hint="Show a Skip Intro control during opening titles. A local preference."
            >
              <Toggle
                label="Offer Skip Intro"
                checked={preferences.skipIntro}
                onChange={(next) => void updatePreference("skipIntro", next)}
              />
            </FieldRow>
            <FieldRow label="Subtitle size" hint="Stored by the backend for every title.">
              <Segmented
                label="Subtitle size"
                options={subtitleOptions}
                value={preferences.subtitleSize}
                onChange={(next) => void updatePreference("subtitleSize", next)}
              />
            </FieldRow>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Appearance</h2>
          <Panel>
            <FieldRow
              label="Reduce motion"
              hint="Turn off artwork drift, focus scaling and page transitions."
            >
              <Toggle
                label="Reduce motion"
                checked={preferences.reduceMotion}
                onChange={(next) => void updatePreference("reduceMotion", next)}
              />
            </FieldRow>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Library</h2>
          <Panel>
            <FieldRow
              label="Media folders"
              hint={`${roots.roots.length} folders · ${totalItems} items indexed · files are never moved`}
            >
              <Link to="/library" className={buttonClass("glass")} data-focusable="true">
                Review folders
              </Link>
            </FieldRow>
            <FieldRow
              label="Downloads"
              hint={
                settings.optionalServices.downloadsEnabled
                  ? "The download service is enabled."
                  : "The download service is disabled in this build."
              }
            >
              <Link to="/downloads" className={buttonClass("glass")} data-focusable="true">
                Open downloads
              </Link>
            </FieldRow>
            <FieldRow
              label="Add a folder"
              hint="Point the scanner at another folder. It only reads."
            >
              <Link to="/setup" className={buttonClass("glass")} data-focusable="true">
                Run setup
              </Link>
            </FieldRow>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>IMDb &amp; OMDb (opt-in)</h2>
          <Panel tone="quiet">
            <FieldRow
              label="IMDb ratings & listings"
              hint="Off by default. When enabled, a title/year or IMDb id is sent only to IMDb dataset files you supply yourself."
            >
              <button
                type="button"
                role="switch"
                aria-checked={false}
                aria-label="IMDb ratings and listings"
                disabled
                className={styles.disabledSwitch}
              >
                Not available
              </button>
            </FieldRow>
            <FieldRow
              label="OMDb age ratings"
              hint="Off by default. When enabled, an IMDb id is sent to OMDb with a key you supply from the secret store. No key is stored in the app."
            >
              <button
                type="button"
                role="switch"
                aria-checked={false}
                aria-label="OMDb age ratings"
                disabled
                className={styles.disabledSwitch}
              >
                Not available
              </button>
            </FieldRow>
            <p className={styles.rootHint}>
              Neither switch is wired to a backend command in this build, so both
              stay off. Posters come from IMDb when available; no API key is used.
              Only a title and year are looked up, and images are cached locally.
              Local NFO and IMDb behaviour is unchanged. Ratings and age ratings:
              IMDb / OMDb.
            </p>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Acquisition</h2>
          <Panel>
            <FieldRow
              label="Download folder"
              hint="Where authorized downloads are stored. qBittorrent and playback both use this folder."
            >
              <div className={styles.rootRow}>
                <input
                  className={styles.rootInput}
                  aria-label="Download folder"
                  value={rootDraft}
                  onChange={(event) => {
                    setRootDraft(event.target.value);
                    setRootSaved(false);
                  }}
                  placeholder={downloadRoot || "Absolute folder path"}
                />
                <button
                  type="button"
                  className={buttonClass("glass")}
                  data-focusable="true"
                  onClick={() => void saveRoot()}
                >
                  Save
                </button>
              </div>
            </FieldRow>
            {rootError ? (
              <p className={styles.rootError} role="alert">
                {rootError}
              </p>
            ) : null}
            {rootSaved && !rootError ? (
              <p className={styles.rootHint} role="status">
                Saved. Restart the download service to apply it to qBittorrent.
              </p>
            ) : null}
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Profile</h2>
          <Panel>
            <FieldRow
              label={activeProfile ? activeProfile.name : "No profile selected"}
              hint="Profiles are stored in the backend settings document."
            >
              <Link to="/profiles" className={buttonClass("glass")} data-focusable="true">
                Switch profile
              </Link>
            </FieldRow>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Diagnostics</h2>
          <Panel>
            <FieldRow label="Library service" hint={runtimeLabel(settings.diagnostics.backendStatus)}>
              <span>{runtimeLabel(settings.diagnostics.backendStatus)}</span>
            </FieldRow>
            <FieldRow label="Playback runtime" hint={runtimeLabel(settings.diagnostics.runtimeStatus)}>
              <span>{runtimeLabel(settings.diagnostics.runtimeStatus)}</span>
            </FieldRow>
            <FieldRow label="Metadata provider" hint="Artwork falls back to the bundled local set.">
              <span>{settings.optionalServices.metadataProvider ?? "None configured"}</span>
            </FieldRow>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>Remote & keyboard</h2>
          <Panel tone="quiet">
            <dl className={styles.keys}>
              <div>
                <dt>Arrow keys</dt>
                <dd>Move focus between titles, rows and controls.</dd>
              </div>
              <div>
                <dt>Enter</dt>
                <dd>Open the focused title or press the focused button.</dd>
              </div>
              <div>
                <dt>Space</dt>
                <dd>Play or pause while a title is playing.</dd>
              </div>
              <div>
                <dt>/</dt>
                <dd>Jump to search from anywhere in the app.</dd>
              </div>
            </dl>
          </Panel>
        </section>

        <section className={styles.group}>
          <h2 className={styles.groupTitle}>About</h2>
          <Panel tone="quiet">
            <p className={styles.about}>
              MEDIA HAVEN runs on this machine and nowhere else. The interface
              talks to the local service over a signed loopback connection.
              There is no account, no subscription and no telemetry.
            </p>
            <p className={styles.version}>Version 0.0.0 · Local-first build</p>
          </Panel>
        </section>
      </div>
    </div>
  );
}
