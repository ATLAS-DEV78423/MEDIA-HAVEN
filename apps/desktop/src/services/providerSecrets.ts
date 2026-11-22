/* Renderer facade for the operator-supplied provider secrets (TMDB token,
 * Prowlarr address and Prowlarr key). A value never survives a save in this
 * process: it goes straight to the host's DPAPI store through the named-command
 * bridge, or to a session-only shim in browser dev. Production builds
 * tree-shake out the DEV branch, so no value reaches sessionStorage,
 * localStorage, a DTO, a log or an error string. */

import { call } from "./transport";

const KINDS = ["tmdb", "prowlarr_url", "prowlarr_key"] as const;
export type ProviderSecretKind = (typeof KINDS)[number];

const DEV_KEYS: Record<ProviderSecretKind, string> = {
  tmdb: "mediahaven.dev.tmdb-token",
  prowlarr_url: "mediahaven.dev.prowlarr-url",
  prowlarr_key: "mediahaven.dev.prowlarr-key",
};

function assertKind(kind: string): asserts kind is ProviderSecretKind {
  if (!KINDS.includes(kind as ProviderSecretKind)) throw new Error("unsupported provider");
}

function hasHostBridge(): boolean {
  if (typeof window === "undefined") return false;
  return Boolean(window.__TAURI_INTERNALS__?.invoke);
}

export async function setProviderSecret(kind: string, value: string): Promise<void> {
  assertKind(kind);
  if (hasHostBridge()) {
    await call("provider_secret_set", { kind, value });
    return;
  }
  if (import.meta.env.DEV) {
    sessionStorage.setItem(DEV_KEYS[kind], value);
    return;
  }
  throw new Error("The desktop host is not available.");
}

export async function hasProviderSecret(kind: string): Promise<boolean> {
  assertKind(kind);
  if (hasHostBridge()) return call<boolean>("provider_secret_has", { kind });
  if (import.meta.env.DEV) return Boolean(sessionStorage.getItem(DEV_KEYS[kind]));
  throw new Error("The desktop host is not available.");
}

export async function clearProviderSecret(kind: string): Promise<void> {
  assertKind(kind);
  if (hasHostBridge()) {
    await call("provider_secret_clear", { kind });
    return;
  }
  if (import.meta.env.DEV) {
    sessionStorage.removeItem(DEV_KEYS[kind]);
    return;
  }
  throw new Error("The desktop host is not available.");
}
