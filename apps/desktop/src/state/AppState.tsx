import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { api, describeError } from "@/services/api";
import type {
  AppSettings,
  DownloadsScreen,
  HomeScreen,
  LibraryRoot,
  LibraryScreen,
  MediaSummary,
  SeriesDetail,
  SettingsPatch,
  WatchlistScreen,
} from "@/services/types";

export type SubtitleSize = "small" | "medium" | "large";

export interface Preferences {
  autoplayNext: boolean;
  skipIntro: boolean;
  subtitleSize: SubtitleSize;
  reduceMotion: boolean;
}

export interface UiProfile {
  id: string;
  name: string;
  initial: string;
  /** Hex colour from the backend; undefined falls back to a tone class. */
  color: string | undefined;
}

export interface LibrarySnapshot {
  home: HomeScreen;
  movies: MediaSummary[];
  shows: SeriesDetail[];
  watchlist: WatchlistScreen;
  roots: LibraryScreen;
  settings: AppSettings;
  downloads: DownloadsScreen;
}

type Status = "loading" | "ready" | "error";

interface LocalState {
  activeProfileId: string | null;
  reduceMotion: boolean;
  skipIntro: boolean;
  recentSearches: string[];
}

const STORAGE_KEY = "mediahaven.ui.v2";

const initialLocal: LocalState = {
  activeProfileId: null,
  reduceMotion: false,
  skipIntro: true,
  recentSearches: [],
};

function readLocal(): LocalState {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return initialLocal;
    const parsed = JSON.parse(raw) as Partial<LocalState>;
    return {
      activeProfileId:
        typeof parsed.activeProfileId === "string" ? parsed.activeProfileId : null,
      reduceMotion: parsed.reduceMotion === true,
      skipIntro: parsed.skipIntro !== false,
      recentSearches: Array.isArray(parsed.recentSearches)
        ? parsed.recentSearches.filter((term): term is string => typeof term === "string").slice(0, 6)
        : [],
    };
  } catch {
    return initialLocal;
  }
}

const BOOT_MAX_ATTEMPTS = 30;
const BOOT_RETRY_MS = 1000;
const BOOT_STARTING_CODES = new Set([
  "backend_unreachable",
  "runtime_unavailable",
  "runtime_unhealthy",
]);

/** True only when a failure means "the backend is not listening yet".
 *  Auth/configuration errors must stop the boot retry loop immediately. */
function isBackendStarting(error: unknown): boolean {
  if (error instanceof TypeError) return true;
  if (typeof error === "object" && error !== null && "code" in error) {
    return typeof error.code === "string" && BOOT_STARTING_CODES.has(error.code);
  }
  return false;
}

async function fetchSnapshot(): Promise<LibrarySnapshot> {
  await api.ready();
  const [home, movies, shows, watchlist, roots, settings, downloads] = await Promise.all([
    api.home(),
    api.movies(),
    api.shows(),
    api.watchlist(),
    api.roots(),
    api.settings(),
    api.downloads(),
  ]);
  return { home, movies, shows, watchlist, roots, settings, downloads };
}

const subtitleValue = (value: string): SubtitleSize =>
  value === "small" || value === "large" ? value : "medium";

const profileColour = /^#[0-9a-f]{6}$/i;

interface AppStateValue {
  status: Status;
  bootError: string | null;
  startupStatus: string | null;
  setStartupStatus: (value: string | null) => void;
  actionError: string | null;
  clearActionError: () => void;
  data: LibrarySnapshot | null;
  profiles: UiProfile[];
  activeProfile: UiProfile | null;
  preferences: Preferences;
  recentSearches: string[];
  isListed: (mediaId: string) => boolean;
  toggleList: (mediaId: string) => Promise<void>;
  selectProfile: (id: string) => void;
  addProfile: (name: string) => Promise<void>;
  removeProfile: (id: string) => Promise<void>;
  updatePreference: <K extends keyof Preferences>(
    key: K,
    value: Preferences[K],
  ) => Promise<void>;
  rememberSearch: (term: string) => void;
  addRoot: (displayName: string, path: string) => Promise<LibraryRoot>;
  scanRoot: (rootId: string) => Promise<{ scanned: number; errors: string[] }>;
  removeRoot: (rootId: string) => Promise<void>;
  saveProgress: (
    mediaId: string,
    positionSeconds: number,
    durationSeconds: number,
  ) => Promise<void>;
  refresh: () => Promise<void>;
  retry: () => Promise<void>;
}

const AppStateContext = createContext<AppStateValue | null>(null);

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [local, setLocal] = useState<LocalState>(readLocal);
  const [data, setData] = useState<LibrarySnapshot | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [bootError, setBootError] = useState<string | null>(null);
  const [startupStatus, setStartupStatus] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // A generation invalidates in-flight retries on unmount or a fresh load.
  const generationRef = useRef(0);
  const retryTimerRef = useRef<number | null>(null);
  const wakeRetryRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(local));
    } catch {
      /* Private windows can refuse storage; the UI still works for this run. */
    }
  }, [local]);

  useEffect(() => {
    document.documentElement.dataset.motion = local.reduceMotion ? "reduced" : "full";
  }, [local.reduceMotion]);

  // Invalidate any pending retry and release a sleeping loop so it can exit.
  const cancelRetry = useCallback(() => {
    generationRef.current += 1;
    if (retryTimerRef.current !== null) {
      window.clearTimeout(retryTimerRef.current);
      retryTimerRef.current = null;
    }
    wakeRetryRef.current?.();
    wakeRetryRef.current = null;
  }, []);

  const waitBeforeRetry = useCallback(
    () =>
      new Promise<void>((resolve) => {
        wakeRetryRef.current = resolve;
        retryTimerRef.current = window.setTimeout(() => {
          retryTimerRef.current = null;
          wakeRetryRef.current = null;
          resolve();
        }, BOOT_RETRY_MS);
      }),
    [],
  );

  const load = useCallback(async () => {
    cancelRetry();
    const generation = generationRef.current;
    for (let attempt = 1; attempt <= BOOT_MAX_ATTEMPTS; attempt += 1) {
      if (generation !== generationRef.current) return;
      try {
        const snapshot = await fetchSnapshot();
        if (generation !== generationRef.current) return;
        setData(snapshot);
        setBootError(null);
        setStatus("ready");
        return;
      } catch (error) {
        if (generation !== generationRef.current) return;
        if (attempt >= BOOT_MAX_ATTEMPTS || !isBackendStarting(error)) {
          setBootError(describeError(error));
          setStatus("error");
          return;
        }
        await waitBeforeRetry();
      }
    }
  }, [cancelRetry, waitBeforeRetry]);

  useEffect(() => {
    // Deferred one task so the first paint is the loading state rather than a
    // cascading render from the fetch.
    const timer = window.setTimeout(() => {
      void load();
    }, 0);
    return () => {
      window.clearTimeout(timer);
      cancelRetry();
    };
  }, [load, cancelRetry]);

  const refresh = useCallback(async () => {
    try {
      const snapshot = await fetchSnapshot();
      setData(snapshot);
      setStatus("ready");
      setBootError(null);
    } catch (error) {
      setActionError(describeError(error));
    }
  }, []);

  const profiles = useMemo<UiProfile[]>(
    () =>
      (data?.settings.profiles ?? []).map((profile) => ({
        id: profile.id,
        name: profile.name,
        initial: profile.name.trim().charAt(0).toUpperCase() || "?",
        color: profileColour.test(profile.avatarColor) ? profile.avatarColor : undefined,
      })),
    [data],
  );

  const activeProfile =
    profiles.find((profile) => profile.id === local.activeProfileId) ?? null;

  const preferences = useMemo<Preferences>(
    () => ({
      autoplayNext: data?.settings.playback.autoplayNext ?? true,
      skipIntro: local.skipIntro,
      subtitleSize: subtitleValue(data?.settings.playback.subtitleSize ?? "medium"),
      reduceMotion: local.reduceMotion,
    }),
    [data, local.skipIntro, local.reduceMotion],
  );

  const selectProfile = useCallback((id: string) => {
    setLocal((current) => ({ ...current, activeProfileId: id }));
  }, []);

  const applySettings = useCallback((settings: AppSettings) => {
    setData((current) => (current ? { ...current, settings } : current));
  }, []);

  const addProfile = useCallback(
    async (name: string) => {
      const trimmed = name.trim().slice(0, 18);
      if (!trimmed || !data) return;
      const profile = {
        id: crypto.randomUUID(),
        name: trimmed,
        // The backend assigns a real colour; the UI never picks or stores one.
        avatarColor: "",
        createdAt: new Date().toISOString(),
      };
      try {
        const settings = await api.settingsSet({
          profiles: [...data.settings.profiles, profile],
        });
        applySettings(settings);
        setLocal((current) => ({ ...current, activeProfileId: profile.id }));
      } catch (error) {
        setActionError(describeError(error));
      }
    },
    [applySettings, data],
  );

  const removeProfile = useCallback(
    async (id: string) => {
      if (!data || data.settings.profiles.length <= 1) return;
      try {
        const settings = await api.settingsSet({
          profiles: data.settings.profiles.filter((profile) => profile.id !== id),
        });
        applySettings(settings);
        if (local.activeProfileId === id) {
          setLocal((current) => ({ ...current, activeProfileId: null }));
        }
      } catch (error) {
        setActionError(describeError(error));
      }
    },
    [applySettings, data, local.activeProfileId],
  );

  const updatePreference = useCallback(
    async <K extends keyof Preferences>(key: K, value: Preferences[K]) => {
      if (key === "reduceMotion" || key === "skipIntro") {
        setLocal((current) => ({ ...current, [key]: Boolean(value) }));
        return;
      }
      const patch: SettingsPatch = {};
      if (key === "autoplayNext") {
        patch.playback = { autoplayNext: Boolean(value) };
      } else if (key === "subtitleSize") {
        patch.playback = { subtitleSize: String(value) };
      }
      try {
        const settings = await api.settingsSet(patch);
        applySettings(settings);
      } catch (error) {
        setActionError(describeError(error));
      }
    },
    [applySettings],
  );

  const isListed = useCallback(
    (mediaId: string) =>
      data?.watchlist.entries.some((entry) => entry.media.id === mediaId) ?? false,
    [data],
  );

  const toggleList = useCallback(
    async (mediaId: string) => {
      if (!data) return;
      const listed = data.watchlist.entries.some((entry) => entry.media.id === mediaId);
      try {
        await (listed ? api.watchlistRemove(mediaId) : api.watchlistAdd(mediaId));
        const watchlist = await api.watchlist();
        setData((current) => (current ? { ...current, watchlist } : current));
      } catch (error) {
        setActionError(describeError(error));
      }
    },
    [data],
  );

  const addRoot = useCallback(
    async (displayName: string, path: string) => {
      const root = await api.rootAdd(displayName, path);
      await refresh();
      return root;
    },
    [refresh],
  );

  const scanRoot = useCallback(
    async (rootId: string) => {
      const outcome = await api.scan(rootId);
      await refresh();
      return { scanned: outcome.scanned, errors: outcome.errors };
    },
    [refresh],
  );

  const removeRoot = useCallback(
    async (rootId: string) => {
      await api.rootRemove(rootId);
      await refresh();
    },
    [refresh],
  );

  const saveProgress = useCallback(
    async (mediaId: string, positionSeconds: number, durationSeconds: number) => {
      try {
        await api.checkpoint(mediaId, positionSeconds, durationSeconds);
      } catch (error) {
        setActionError(describeError(error));
      }
    },
    [],
  );

  const rememberSearch = useCallback((term: string) => {
    const trimmed = term.trim();
    if (!trimmed) return;
    setLocal((current) => ({
      ...current,
      recentSearches: [trimmed, ...current.recentSearches.filter((t) => t !== trimmed)].slice(0, 6),
    }));
  }, []);

  const clearActionError = useCallback(() => setActionError(null), []);

  const value = useMemo<AppStateValue>(
    () => ({
      status,
      bootError,
      startupStatus,
      setStartupStatus,
      actionError,
      clearActionError,
      data,
      profiles,
      activeProfile,
      preferences,
      recentSearches: local.recentSearches,
      isListed,
      toggleList,
      selectProfile,
      addProfile,
      removeProfile,
      updatePreference,
      rememberSearch,
      addRoot,
      scanRoot,
      removeRoot,
      saveProgress,
      refresh,
      retry: load,
    }),
    [
      status,
      bootError,
      startupStatus,
      setStartupStatus,
      actionError,
      clearActionError,
      data,
      profiles,
      activeProfile,
      preferences,
      local.recentSearches,
      isListed,
      toggleList,
      selectProfile,
      addProfile,
      removeProfile,
      updatePreference,
      rememberSearch,
      addRoot,
      scanRoot,
      removeRoot,
      saveProgress,
      refresh,
      load,
    ],
  );

  return <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>;
}

export function useAppState(): AppStateValue {
  const value = useContext(AppStateContext);
  if (!value) {
    throw new Error("useAppState must be used inside AppStateProvider");
  }
  return value;
}
