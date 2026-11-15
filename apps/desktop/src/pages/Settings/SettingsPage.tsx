import type { AppSettings, SettingsPatch } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { PageHeader, ErrorState } from "@/components/feedback/States";
import { StatusPill } from "@/components/cards/ProgressBar";
import styles from "./SettingsPage.module.css";

const SECTIONS = [
  { id: "general", label: "General" },
  { id: "library", label: "Library folders" },
  { id: "playback", label: "Playback" },
  { id: "storage", label: "Storage" },
  { id: "services", label: "Optional services" },
  { id: "diagnostics", label: "Diagnostics" },
  { id: "about", label: "About" },
] as const;

function formatBytes(bytes: number | null): string {
  if (bytes === null) return "No limit";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  readonly checked: boolean;
  readonly onChange: (next: boolean) => void;
  readonly label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`${styles.toggle} ${checked ? styles.toggleOn : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className={styles.toggleKnob} />
    </button>
  );
}

export function SettingsPage() {
  const settings = useCommand<AppSettings>(
    async () => (await getCommands()).settings_get(),
    [],
  );

  const apply = async (patch: SettingsPatch) => {
    const commands = await getCommands();
    await commands.settings_set(patch);
  };

  if (settings.status === "error") {
    return <ErrorState error={settings.error} onRetry={settings.reload} />;
  }

  if (settings.status === "loading") {
    return <PageHeader title="Settings" subtitle="Loading…" />;
  }

  const data = settings.data;
  const runtime = data.diagnostics.runtimeStatus;
  const backend = data.diagnostics.backendStatus;

  const runtimeTone =
    runtime.state === "ready"
      ? "available"
      : runtime.state === "unhealthy" || runtime.state === "absent"
        ? "missing"
        : "neutral";

  const backendTone =
    backend.state === "ready"
      ? "available"
      : backend.state === "unhealthy" || backend.state === "absent"
        ? "missing"
        : "neutral";

  return (
    <>
      <PageHeader
        title="Settings"
        subtitle="Everything here is stored on this machine."
      />
      <div className={styles.layout}>
        <nav className={styles.nav} aria-label="Settings sections">
          {SECTIONS.map((section) => (
            <a
              key={section.id}
              className={styles.navItem}
              href={`#${section.id}`}
            >
              {section.label}
            </a>
          ))}
        </nav>

        <div className={styles.panels}>
          <section className={styles.panel} id="general">
            <h2 className={styles.panelTitle}>General</h2>
            <p className={styles.panelBody}>
              MEDIA_HAVEN has no account, no subscription and no telemetry. It
              never uploads your library, and it never deletes your original
              media.
            </p>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Network access</div>
                  <div className={styles.rowHint}>
                    Optional metadata providers are the only outbound requests.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <StatusPill tone="offline">Local only</StatusPill>
                </div>
              </div>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Metadata provider</div>
                  <div className={styles.rowHint}>
                    Optional. The library works fully without one.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <span className={styles.value}>
                    {data.optionalServices.metadataProvider ?? "None configured"}
                  </span>
                </div>
              </div>
            </div>
          </section>

          <section className={styles.panel} id="library">
            <h2 className={styles.panelTitle}>Library folders</h2>
            <p className={styles.panelBody}>
              Folders you have chosen to read. They are mounted read-only, so
              MEDIA_HAVEN cannot change the files inside them.
            </p>
            <div className={styles.rows}>
              {data.libraryRoots.map((root) => (
                <div className={styles.row} key={root.id}>
                  <div>
                    <div className={styles.rowLabel}>{root.displayName}</div>
                    <div className={styles.rowHint}>
                      {root.itemCount.toLocaleString()} items
                    </div>
                  </div>
                  <div className={styles.rowControl}>
                    <StatusPill
                      tone={
                        root.status === "ok"
                          ? "available"
                          : root.status === "offline"
                            ? "offline"
                            : "missing"
                      }
                    >
                      {root.status === "ok"
                        ? "Available"
                        : root.status === "scanning"
                          ? "Scanning"
                          : root.status === "offline"
                            ? "Offline"
                            : "Error"}
                    </StatusPill>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className={styles.panel} id="playback">
            <h2 className={styles.panelTitle}>Playback</h2>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Preferred language</div>
                  <div className={styles.rowHint}>
                    Used to pick audio and subtitle tracks automatically.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <select
                    className={styles.select}
                    aria-label="Preferred language"
                    value={data.playback.preferredLanguage}
                    onChange={(event) =>
                      void apply({
                        playback: { preferredLanguage: event.target.value },
                      })
                    }
                  >
                    <option value="en">English</option>
                    <option value="de">German</option>
                    <option value="fr">French</option>
                    <option value="es">Spanish</option>
                    <option value="it">Italian</option>
                  </select>
                </div>
              </div>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Subtitle size</div>
                  <div className={styles.rowHint}>
                    Subtitles never render below 16 px in TV mode.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <select
                    className={styles.select}
                    aria-label="Subtitle size"
                    value={data.playback.subtitleSize}
                    onChange={(event) =>
                      void apply({
                        playback: {
                          subtitleSize: event.target.value as AppSettings["playback"]["subtitleSize"],
                        },
                      })
                    }
                  >
                    <option value="small">Small</option>
                    <option value="medium">Medium</option>
                    <option value="large">Large</option>
                  </select>
                </div>
              </div>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Autoplay next episode</div>
                  <div className={styles.rowHint}>
                    Off by default. A prompt appears first when it is on.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <Toggle
                    label="Autoplay next episode"
                    checked={data.playback.autoplayNext}
                    onChange={(next) =>
                      void apply({ playback: { autoplayNext: next } })
                    }
                  />
                </div>
              </div>
            </div>
          </section>

          <section className={styles.panel} id="storage">
            <h2 className={styles.panelTitle}>Storage</h2>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Managed media location</div>
                  <div className={styles.rowHint}>
                    Only used by optional features that download.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <span className={styles.value}>
                    {data.storage.rootDisplayName}
                  </span>
                </div>
              </div>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Storage limit</div>
                  <div className={styles.rowHint}>
                    Nothing is deleted automatically. Cleanup always asks
                    first.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <span className={styles.value}>
                    {formatBytes(data.storage.limitBytes)}
                  </span>
                </div>
              </div>
            </div>
          </section>

          <section className={styles.panel} id="services">
            <h2 className={styles.panelTitle}>Optional services</h2>
            <p className={styles.panelBody}>
              These are separate, opt-in integrations. MEDIA_HAVEN ships no
              content sources and works without any of them.
            </p>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Download provider</div>
                  <div className={styles.rowHint}>
                    Requires your own lawful provider configuration.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <StatusPill
                    tone={data.optionalServices.downloadsEnabled ? "available" : "offline"}
                  >
                    {data.optionalServices.downloadsEnabled
                      ? "Enabled"
                      : "Disabled"}
                  </StatusPill>
                </div>
              </div>
            </div>
          </section>

          <section className={styles.panel} id="diagnostics">
            <h2 className={styles.panelTitle}>Diagnostics</h2>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Container runtime</div>
                  <div className={styles.rowHint}>
                    The isolated workspace the backend runs in.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <StatusPill tone={runtimeTone}>{runtime.state}</StatusPill>
                </div>
              </div>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Backend</div>
                  <div className={styles.rowHint}>
                    The service that holds your library and watch history.
                  </div>
                </div>
                <div className={styles.rowControl}>
                  <StatusPill tone={backendTone}>{backend.state}</StatusPill>
                </div>
              </div>
            </div>
          </section>

          <section className={styles.panel} id="about">
            <h2 className={styles.panelTitle}>About</h2>
            <p className={styles.panelBody}>
              MEDIA_HAVEN is free and open source. Third-party licences for
              everything bundled with the installer are listed here and are
              bundled with the release.
            </p>
            <div className={styles.rows}>
              <div className={styles.row}>
                <div>
                  <div className={styles.rowLabel}>Version</div>
                </div>
                <div className={styles.rowControl}>
                  <span className={styles.value}>0.0.0 (pre-implementation)</span>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </>
  );
}
