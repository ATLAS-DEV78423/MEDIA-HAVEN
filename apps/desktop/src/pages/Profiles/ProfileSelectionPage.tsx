import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { ActionButton, buttonClass } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import { useAppState } from "@/state/AppState";
import styles from "./profiles.module.css";

type Dialog =
  | { kind: "none" }
  | { kind: "add" }
  | { kind: "remove"; id: string; name: string };

export function ProfileSelectionPage() {
  const { profiles, activeProfile, selectProfile, addProfile, removeProfile } =
    useAppState();
  const navigate = useNavigate();
  const [managing, setManaging] = useState(false);
  const [dialog, setDialog] = useState<Dialog>({ kind: "none" });
  const [name, setName] = useState("");

  useEffect(() => {
    if (dialog.kind === "none") return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setDialog({ kind: "none" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialog.kind]);

  const choose = (id: string) => {
    if (managing) return;
    selectProfile(id);
    navigate("/");
  };

  const create = async () => {
    if (!name.trim()) return;
    await addProfile(name);
    setName("");
    setDialog({ kind: "none" });
    navigate("/");
  };

  return (
    <div className={styles.page}>
      <div className={styles.background} aria-hidden="true">
        <img src="/artwork/br-hero.jpg" alt="" className={styles.bgArt} />
        <div className={styles.scrim} />
      </div>

      <header className={styles.brandBar}>
        <span className={styles.wordmark}>MEDIA HAVEN</span>
        {activeProfile ? (
          <Link to="/" className={styles.exit} data-focusable="true">
            Back to browsing
          </Link>
        ) : null}
      </header>

      <main className={styles.stage}>
        <section className={styles.panel} aria-labelledby="profiles-title">
          <h1 id="profiles-title" className={styles.title}>
            Who&apos;s watching?
          </h1>
          <p className={styles.sub}>
            Profiles are stored in the local backend. They never leave this
            machine.
          </p>

          <div className={styles.grid}>
            {profiles.map((profile) => (
              <div key={profile.id} className={styles.cell}>
                <button
                  type="button"
                  className={`${styles.profile} ${
                    activeProfile?.id === profile.id ? styles.profileActive : ""
                  }`}
                  onClick={() => choose(profile.id)}
                  aria-label={`Watch as ${profile.name}`}
                  data-focusable="true"
                >
                  <span
                    className={`${styles.avatar} ${profile.color ? "" : styles.tone1}`}
                    style={profile.color ? { background: profile.color } : undefined}
                  >
                    {profile.initial}
                  </span>
                  <span className={styles.name}>{profile.name}</span>
                  <span className={styles.kind}>Profile</span>
                </button>
                {managing && profiles.length > 1 ? (
                  <button
                    type="button"
                    className={styles.remove}
                    aria-label={`Remove ${profile.name}`}
                    onClick={() =>
                      setDialog({
                        kind: "remove",
                        id: profile.id,
                        name: profile.name,
                      })
                    }
                    data-focusable="true"
                  >
                    <Icon name="close" />
                  </button>
                ) : null}
              </div>
            ))}

            {profiles.length < 6 ? (
              <button
                type="button"
                className={`${styles.profile} ${styles.add}`}
                onClick={() => setDialog({ kind: "add" })}
                data-focusable="true"
              >
                <span className={styles.avatarAdd}>
                  <Icon name="plus" />
                </span>
                <span className={styles.name}>Add profile</span>
                <span className={styles.kind}>Up to six</span>
              </button>
            ) : null}
          </div>

          <div className={styles.footer}>
            <ActionButton
              variant="quiet"
              icon={managing ? "check" : "sliders"}
              onClick={() => setManaging((current) => !current)}
            >
              {managing ? "Done managing" : "Manage profiles"}
            </ActionButton>
            <Link to="/setup" className={buttonClass("quiet")} data-focusable="true">
              Media folders
            </Link>
          </div>
        </section>
      </main>

      <p className={styles.legal}>
        A local appliance. No account, no cloud profile, no telemetry.
      </p>

      {dialog.kind !== "none" ? (
        <div
          className={styles.overlay}
          role="dialog"
          aria-modal="true"
          aria-labelledby="dialog-title"
        >
          <div
            className={styles.dialog}
            onKeyDown={(event) => {
              if (event.key === "Escape") setDialog({ kind: "none" });
            }}
          >
            {dialog.kind === "add" ? (
              <>
                <h2 id="dialog-title" className={styles.dialogTitle}>
                  New profile
                </h2>
                <p className={styles.dialogBody}>
                  A name and a colour. The profile is saved by the backend and
                  stays on this machine.
                </p>
                <label className={styles.field}>
                  <span className={styles.fieldLabel}>Name</span>
                  <input
                    className={styles.input}
                    value={name}
                    maxLength={18}
                    autoFocus
                    placeholder="e.g. Rowan"
                    onChange={(event) => setName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void create();
                    }}
                  />
                </label>
                <div className={styles.dialogActions}>
                  <ActionButton variant="quiet" onClick={() => setDialog({ kind: "none" })}>
                    Cancel
                  </ActionButton>
                  <ActionButton
                    variant="solid"
                    onClick={() => void create()}
                    disabled={!name.trim()}
                  >
                    Create profile
                  </ActionButton>
                </div>
              </>
            ) : (
              <>
                <h2 id="dialog-title" className={styles.dialogTitle}>
                  Remove {dialog.name}?
                </h2>
                <p className={styles.dialogBody}>
                  The profile disappears from the backend settings. Media files,
                  folders and downloads on this machine are not touched.
                </p>
                <div className={styles.dialogActions}>
                  <ActionButton variant="quiet" onClick={() => setDialog({ kind: "none" })}>
                    Keep profile
                  </ActionButton>
                  <ActionButton
                    variant="solid"
                    onClick={() => {
                      void removeProfile(dialog.id);
                      setDialog({ kind: "none" });
                    }}
                  >
                    Remove profile
                  </ActionButton>
                </div>
              </>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
