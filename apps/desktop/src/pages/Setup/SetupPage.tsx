import { useState } from "react";
import { Link } from "react-router-dom";

import { ActionButton, buttonClass } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import { describeError } from "@/services/api";
import { useAppState } from "@/state/AppState";
import styles from "./setup.module.css";

type Step = 1 | 2 | 3;

const STEPS: { step: Step; label: string }[] = [
  { step: 1, label: "Folder" },
  { step: 2, label: "Scan" },
  { step: 3, label: "Ready" },
];

const folderName = (path: string): string => {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? "Media";
};

export function SetupPage() {
  const { data, activeProfile, addRoot, scanRoot } = useAppState();
  const [step, setStep] = useState<Step>(1);
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scanned, setScanned] = useState<number | null>(null);
  const [scanErrors, setScanErrors] = useState<string[]>([]);

  const startScan = async () => {
    const target = path.trim();
    if (!target || busy) return;
    setBusy(true);
    setError(null);
    setScanErrors([]);
    setScanned(null);
    setStep(2);
    try {
      const root = await addRoot(folderName(target), target);
      const result = await scanRoot(root.id);
      setScanned(result.scanned);
      setScanErrors(result.errors);
      setStep(3);
    } catch (failure) {
      setError(describeError(failure));
      setStep(1);
    } finally {
      setBusy(false);
    }
  };

  const counts = data?.roots.counts;

  return (
    <div className={styles.page}>
      <div className={styles.background} aria-hidden="true">
        <img src="/artwork/dune-hero.jpg" alt="" className={styles.bgArt} />
        <div className={styles.scrim} />
      </div>

      <header className={styles.top}>
        <span className={styles.wordmark}>MEDIA HAVEN</span>
        <Link to="/" className={styles.skip} data-focusable="true">
          Skip setup
        </Link>
      </header>

      <main className={styles.stage}>
        <section className={styles.panel} aria-labelledby="setup-title">
          <ol className={styles.steps} aria-label="Setup progress">
            {STEPS.map((item) => (
              <li
                key={item.step}
                className={`${styles.stepItem} ${
                  item.step === step ? styles.stepCurrent : ""
                } ${item.step < step ? styles.stepDone : ""}`}
                aria-current={item.step === step ? "step" : undefined}
              >
                <span className={styles.stepBar} />
                <span className={styles.stepLabel}>{item.label}</span>
              </li>
            ))}
          </ol>

          {step === 1 ? (
            <>
              <h1 id="setup-title" className={styles.title}>
                Bring your library in
              </h1>
              <p className={styles.lede}>
                Point MEDIA HAVEN at the folder that holds your films and series.
                The backend reads it, indexes what it finds and never moves a
                file.
              </p>

              <label className={styles.field}>
                <span className={styles.fieldLabel}>Media folder</span>
                <span className={styles.inputRow}>
                  <Icon name="folder" className={styles.inputIcon} />
                  <input
                    className={styles.input}
                    value={path}
                    spellCheck={false}
                    autoFocus
                    placeholder="D:\Media"
                    onChange={(event) => setPath(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void startScan();
                    }}
                  />
                </span>
              </label>

              {error ? <p className={styles.error}>{error}</p> : null}

              <p className={styles.fineprint}>
                Files are indexed in place. Nothing is copied, renamed or
                deleted. You can add more folders later.
              </p>

              <div className={styles.actions}>
                <ActionButton
                  variant="solid"
                  icon="arrowRight"
                  disabled={!path.trim() || busy}
                  onClick={() => void startScan()}
                >
                  Scan this folder
                </ActionButton>
                <Link to="/" className={buttonClass("quiet")} data-focusable="true">
                  Not now
                </Link>
              </div>
            </>
          ) : null}

          {step === 2 ? (
            <>
              <h1 id="setup-title" className={styles.title}>
                Reading {path}
              </h1>
              <p className={styles.lede}>
                The backend is walking the folder. Nothing is uploaded and no
                catalogue service is contacted.
              </p>

              <div className={styles.progressBlock}>
                <div className={styles.progressHead}>
                  <span>Indexing</span>
                  <span className={styles.progressValue}>In progress</span>
                </div>
                <span className={styles.scanBar} aria-hidden="true" />
              </div>

              <p className={styles.fineprint}>
                A large folder can take a moment. The scan is interruptible and
                safe to re-run.
              </p>
            </>
          ) : null}

          {step === 3 ? (
            <>
              <span className={styles.doneIcon} aria-hidden="true">
                <Icon name="checkCircle" />
              </span>
              <h1 id="setup-title" className={styles.title}>
                Your library is ready
              </h1>
              <p className={styles.lede}>
                {scanned ?? 0} files indexed from {path}. The backend now serves
                them to every screen.
              </p>

              <dl className={styles.counts}>
                <div>
                  <dt>Films</dt>
                  <dd>{counts?.movies ?? 0}</dd>
                </div>
                <div>
                  <dt>Series</dt>
                  <dd>{counts?.shows ?? 0}</dd>
                </div>
                <div>
                  <dt>Episodes</dt>
                  <dd>{counts?.episodes ?? 0}</dd>
                </div>
              </dl>

              {scanErrors.length > 0 ? (
                <p className={styles.error}>
                  {scanErrors.length} files could not be read: {scanErrors[0]}
                </p>
              ) : null}

              <div className={styles.actions}>
                <Link
                  to={activeProfile ? "/" : "/profiles"}
                  className={buttonClass("solid")}
                  data-focusable="true"
                >
                  {activeProfile ? "Start watching" : "Choose a profile"}
                </Link>
                <ActionButton
                  variant="quiet"
                  onClick={() => {
                    setPath("");
                    setStep(1);
                  }}
                >
                  Add another folder
                </ActionButton>
              </div>
            </>
          ) : null}
        </section>

        <p className={styles.legal}>
          Local-first · Nothing leaves this machine · No account required
        </p>
      </main>
    </div>
  );
}
