import { useEffect, useRef, type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { PrimaryNavigation } from "./PrimaryNavigation";
import type { RuntimeStatus } from "@/types/media";
import styles from "./shell.module.css";

export interface AppShellProps {
  readonly runtimeStatus: RuntimeStatus;
  readonly downloadsEnabled: boolean;
  readonly children: ReactNode;
}

export function AppShell({
  runtimeStatus,
  downloadsEnabled,
  children,
}: AppShellProps) {
  const location = useLocation();
  const mainRef = useRef<HTMLElement | null>(null);
  const firstRender = useRef(true);

  // On navigation, move focus into the new page so a keyboard or remote user
  // is never left with focus on a link that no longer exists.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    mainRef.current?.focus({ preventScroll: true });
    mainRef.current?.scrollTo({ top: 0 });
  }, [location.pathname]);

  return (
    <div className={styles.shell}>
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <PrimaryNavigation
        runtimeStatus={runtimeStatus}
        downloadsEnabled={downloadsEnabled}
      />
      <main
        id="main-content"
        ref={mainRef}
        className={styles.main}
        tabIndex={-1}
      >
        <div className={styles.canvas}>{children}</div>
      </main>
    </div>
  );
}
