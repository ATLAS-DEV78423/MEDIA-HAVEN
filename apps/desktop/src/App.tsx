import { useMemo } from "react";
import { Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/navigation/AppShell";
import { FocusManager } from "@/components/navigation/FocusManager";
import { MediaActionsProvider } from "@/hooks/useMediaActions";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import type { AppSettings } from "@/types/media";

import { HomePage } from "@/pages/Home/HomePage";
import { MoviesPage } from "@/pages/Movies/MoviesPage";
import { ShowsPage } from "@/pages/Shows/ShowsPage";
import { LibraryPage } from "@/pages/Library/LibraryPage";
import { WatchlistPage } from "@/pages/Watchlist/WatchlistPage";
import { DownloadsPage } from "@/pages/Downloads/DownloadsPage";
import { SearchPage } from "@/pages/Search/SearchPage";
import { SettingsPage } from "@/pages/Settings/SettingsPage";
import { SetupPage } from "@/pages/Setup/SetupPage";

function Shell({ children }: { readonly children: React.ReactNode }) {
  const settings = useCommand<AppSettings>(
    async () => (await getCommands()).settings_get(),
    [],
  );

  const runtimeStatus =
    settings.status === "ready"
      ? settings.data.diagnostics.runtimeStatus
      : { state: "unknown" as const };

  const downloadsEnabled =
    settings.status === "ready"
      ? settings.data.optionalServices.downloadsEnabled
      : false;

  return (
    <AppShell
      runtimeStatus={runtimeStatus}
      downloadsEnabled={downloadsEnabled}
    >
      {children}
    </AppShell>
  );
}

export function App() {
  const settings = useCommand<AppSettings>(
    async () => (await getCommands()).settings_get(),
    [],
  );

  const watchlistIds = useMemo<readonly string[]>(
    () => (settings.status === "ready" ? [] : []),
    [settings],
  );

  return (
    <FocusManager>
      <MediaActionsProvider initialWatchlistIds={watchlistIds}>
        <Routes>
          <Route
            path="*"
            element={
              <Shell>
                <Routes>
                  <Route path="/" element={<HomePage />} />
                  <Route path="/movies" element={<MoviesPage />} />
                  <Route path="/shows" element={<ShowsPage />} />
                  <Route path="/library" element={<LibraryPage />} />
                  <Route path="/watchlist" element={<WatchlistPage />} />
                  <Route path="/downloads" element={<DownloadsPage />} />
                  <Route path="/search" element={<SearchPage />} />
                  <Route path="/settings" element={<SettingsPage />} />
                  <Route path="/setup" element={<SetupPage />} />
                  <Route path="*" element={<HomePage />} />
                </Routes>
              </Shell>
            }
          />
        </Routes>
      </MediaActionsProvider>
    </FocusManager>
  );
}
