import { useEffect } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";

import { Nav } from "./components/nav/Nav";
import { ActionButton } from "./components/ui/Controls";
import { HomePage } from "./pages/Home/HomePage";
import { MoviesPage } from "./pages/Movies/MoviesPage";
import { ShowsPage } from "./pages/Shows/ShowsPage";
import { AnimePage } from "./pages/Anime/AnimePage";
import { MediaDetailsPage } from "./pages/Details/MediaDetailsPage";
import { LibraryPage } from "./pages/Library/LibraryPage";
import { WatchlistPage } from "./pages/Watchlist/WatchlistPage";
import { QueuePage } from "./pages/Queue/QueuePage";
import { DownloadsPage } from "./pages/Downloads/DownloadsPage";
import { SearchPage } from "./pages/Search/SearchPage";
import { SettingsPage } from "./pages/Settings/SettingsPage";
import { SetupPage } from "./pages/Setup/SetupPage";
import { ProfileSelectionPage } from "./pages/Profiles/ProfileSelectionPage";
import { PlayerPage } from "./pages/Player/PlayerPage";
import { useSpatialFocus } from "./hooks/useSpatialFocus";
import { useBackNavigation } from "./hooks/useBackNavigation";
import { apiBase } from "./services/api";
import { useAppState } from "./state/AppState";
import styles from "./app.module.css";

function ScrollToTop() {
  const { pathname } = useLocation();
  useEffect(() => {
    window.scrollTo({ top: 0, behavior: "auto" });
  }, [pathname]);
  return null;
}

function BootScreen() {
  return (
    <div className={styles.gate} role="status" aria-live="polite">
      <span className={styles.mark}>MEDIA HAVEN</span>
      <span className={styles.spinner} aria-hidden="true" />
      <p className={styles.gateBody}>Opening the local library…</p>
    </div>
  );
}

function ServiceDown({
  message,
  onRetry,
}: {
  message: string | null;
  onRetry: () => void;
}) {
  return (
    <div className={styles.gate}>
      <span className={styles.mark}>MEDIA HAVEN</span>
      <div className={styles.gatePanel}>
        <h1 className={styles.gateTitle}>The library service is not answering</h1>
        <p className={styles.gateBody}>
          {message ?? "The backend did not respond."} The interface has no offline
          copy of your catalogue, so it will not show invented titles.
        </p>
        <p className={styles.gateBody}>
          Start the backend, then retry:
          <code className={styles.code}>
            MEDIAHAVEN_DEV=1 cargo run -p mediahaven-core
          </code>
        </p>
        <p className={styles.gateMeta}>Expected at {apiBase}</p>
        <div className={styles.gateActions}>
          <ActionButton variant="solid" icon="arrowRight" onClick={onRetry}>
            Retry
          </ActionButton>
        </div>
      </div>
    </div>
  );
}

export function App() {
  const {
    status,
    data,
    bootError,
    retry,
    activeProfile,
    actionError,
    clearActionError,
  } = useAppState();
  const { pathname, key } = useLocation();
  useSpatialFocus();
  useBackNavigation();

  if (status === "loading") return <BootScreen />;
  if (status === "error" || !data) {
    return <ServiceDown message={bootError} onRetry={() => void retry()} />;
  }

  const chromeless =
    pathname === "/profiles" ||
    pathname === "/setup" ||
    pathname.startsWith("/player");

  if (!activeProfile && pathname !== "/profiles" && pathname !== "/setup") {
    return <Navigate to="/profiles" replace />;
  }

  return (
    <>
      <ScrollToTop />
      {!chromeless && activeProfile ? <Nav /> : null}
      <main id="main">
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/movies" element={<MoviesPage />} />
          <Route path="/shows" element={<ShowsPage />} />
          <Route path="/anime" element={<AnimePage />} />
          <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
          <Route path="/player/:mediaId" element={<PlayerPage key={key} />} />
          <Route path="/library" element={<LibraryPage />} />
          <Route path="/watchlist" element={<WatchlistPage />} />
          <Route path="/queue" element={<QueuePage />} />
          <Route path="/downloads" element={<DownloadsPage />} />
          <Route path="/search" element={<SearchPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/profiles" element={<ProfileSelectionPage />} />
          <Route path="*" element={<HomePage />} />
        </Routes>
      </main>

      {actionError ? (
        <div className={styles.toast} role="alert">
          <span className={styles.toastText}>{actionError}</span>
          <button
            type="button"
            className={styles.toastDismiss}
            onClick={clearActionError}
            data-focusable="true"
          >
            Dismiss
          </button>
        </div>
      ) : null}
    </>
  );
}
