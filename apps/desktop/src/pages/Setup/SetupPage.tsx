import type { AppSettings } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { PageHeader, ErrorState } from "@/components/feedback/States";
import { StatusPill } from "@/components/cards/ProgressBar";
import styles from "./SetupPage.module.css";

type StepState = "done" | "blocked" | "pending";

interface Step {
  readonly title: string;
  readonly body: string;
  readonly state: StepState;
  readonly status?: string;
}

export function SetupPage() {
  const settings = useCommand<AppSettings>(
    async () => (await getCommands()).settings_get(),
    [],
  );

  if (settings.status === "error") {
    return <ErrorState error={settings.error} onRetry={settings.reload} />;
  }

  if (settings.status === "loading") {
    return <PageHeader title="Setup" subtitle="Checking this machine…" />;
  }

  const data = settings.data;
  const runtime = data.diagnostics.runtimeStatus;
  const backend = data.diagnostics.backendStatus;

  const runtimeReady = runtime.state === "ready";
  const backendReady = backend.state === "ready";
  const rootsAdded = data.libraryRoots.length > 0;

  const steps: readonly Step[] = [
    {
      title: "This machine",
      body: "MEDIA_HAVEN runs on Windows with a local container runtime. Nothing is installed without your say-so.",
      state: "done",
      status: "Checked",
    },
    {
      title: "Container runtime",
      body:
        runtime.state === "unhealthy"
          ? ("message" in runtime ? runtime.message : "The runtime is running but not healthy.")
          : "The backend runs inside an isolated Docker workspace, so it cannot read anything it was not given.",
      state: runtimeReady ? "done" : "blocked",
      status: runtime.state,
    },
    {
      title: "Backend service",
      body:
        "The service that holds your library, watch history and watchlist. It runs in the container workspace, never as a loose process on your machine.",
      state: backendReady ? "done" : "blocked",
      status: backend.state,
    },
    {
      title: "Media folders",
      body: rootsAdded
        ? `${data.libraryRoots.length} folder${data.libraryRoots.length === 1 ? "" : "s"} added. They are mounted read-only, so your files are never modified.`
        : "Add a folder that holds your films and series. MEDIA_HAVEN reads them where they are — it does not move, rename or delete them.",
      state: rootsAdded ? "done" : "pending",
      status: rootsAdded ? `${data.libraryRoots.length} added` : "Not added",
    },
    {
      title: "Optional services",
      body: "Metadata providers and downloads are separate, opt-in integrations. MEDIA_HAVEN ships no content sources and needs none of them to work.",
      state: "done",
      status: "Optional",
    },
  ];

  return (
    <>
      <PageHeader title="Setup" />
      <div className={styles.page}>
        <p className={styles.intro}>
          MEDIA_HAVEN is a local appliance: it reads the folders you choose,
          keeps everything else on this machine, and never uploads your library.
          These are the steps to get it running.
        </p>

        <ol className={styles.steps}>
          {steps.map((step, index) => (
            <li className={styles.step} key={step.title}>
              <span
                className={`${styles.stepIndex} ${
                  step.state === "done"
                    ? styles.stepDone
                    : step.state === "blocked"
                      ? styles.stepBlocked
                      : ""
                }`}
                aria-hidden="true"
              >
                {index + 1}
              </span>
              <span>
                <span className={styles.stepTitle}>{step.title}</span>
                <span className={styles.stepBody}>{step.body}</span>
              </span>
              <StatusPill
                tone={
                  step.state === "done"
                    ? "available"
                    : step.state === "blocked"
                      ? "missing"
                      : "offline"
                }
              >
                {step.status}
              </StatusPill>
            </li>
          ))}
        </ol>

        <div className={styles.notes}>
          <strong>What MEDIA_HAVEN will never do:</strong>
          <ul className={styles.notesList}>
            <li>Delete or modify a file in a folder you added.</li>
            <li>Upload your library, your filenames or your watch history.</li>
            <li>Ask for an account, a subscription or a card.</li>
            <li>Change Windows settings or install anything without asking.</li>
          </ul>
        </div>
      </div>
    </>
  );
}
