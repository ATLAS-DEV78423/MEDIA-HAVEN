import type { DownloadsScreen } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { PageHeader, EmptyState, ErrorState } from "@/components/feedback/States";
import { ProgressBar, StatusPill } from "@/components/cards/ProgressBar";
import styles from "@/components/feedback/page.module.css";

export function DownloadsPage() {
  const downloads = useCommand<DownloadsScreen>(
    async () => (await getCommands()).downloads_list(),
    [],
  );

  if (downloads.status === "error") {
    return <ErrorState error={downloads.error} onRetry={downloads.reload} />;
  }

  if (downloads.status === "loading") {
    return <PageHeader title="Downloads" subtitle="Loading…" />;
  }

  const data = downloads.data;

  if (!data.enabled) {
    return (
      <>
        <PageHeader
          title="Downloads"
          subtitle="Optional, separate, opt-in."
        />
        <EmptyState
          title="Downloads are not enabled"
          body="MEDIA_HAVEN does not ship any content sources. Downloads are an optional integration you configure yourself, and only after a separate legal and security review. Nothing is downloaded unless you explicitly set up a provider."
        />
      </>
    );
  }

  return (
    <>
      <PageHeader
        title="Downloads"
        subtitle={`${data.tasks.length} task${data.tasks.length === 1 ? "" : "s"}`}
      />

      {data.storageLimitBytes !== null ? (
        <div className={styles.banner}>
          <div>
            <span className={styles.bannerTitle}>Storage</span>
            <span>
              {formatBytes(data.storageUsedBytes)} of {formatBytes(data.storageLimitBytes)} used.
            </span>
          </div>
        </div>
      ) : null}

      {data.tasks.length === 0 ? (
        <EmptyState
          title="No downloads"
          body="When a provider is configured, active and queued downloads appear here."
        />
      ) : (
        <div className={styles.section}>
          <div
            style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}
          >
            {data.tasks.map((task) => (
              <div
                key={task.id}
                style={{
                  padding: "var(--space-5)",
                  borderRadius: "var(--radius-card)",
                  backgroundColor: "var(--surface-hover)",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: "var(--space-4)", alignItems: "center" }}>
                  <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>
                    {task.title}
                  </span>
                  <StatusPill
                    tone={
                      task.state === "available"
                        ? "available"
                        : task.state === "failed"
                          ? "missing"
                          : "neutral"
                    }
                  >
                    {task.state}
                  </StatusPill>
                </div>
                {task.state === "downloading" || task.state === "verifying" ? (
                  <div style={{ marginTop: "var(--space-3)" }}>
                    <ProgressBar value={task.progress} label={task.title} />
                  </div>
                ) : null}
                {task.errorMessage ? (
                  <div style={{ marginTop: "var(--space-2)", fontSize: "var(--text-meta)", color: "var(--text-tertiary)" }}>
                    {task.errorMessage}
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}

function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}
