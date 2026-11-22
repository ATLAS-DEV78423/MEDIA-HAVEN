/* Renderer facade for one-click source seeding. The host reads the Prowlarr
 * address and key from its own secret store, so neither value reaches this
 * module, a DTO, a log or an error string. */

import { call } from "./transport";

export interface SeedSourcesResult {
  added: string[];
  skipped: string[];
}

function hasHostBridge(): boolean {
  if (typeof window === "undefined") return false;
  return Boolean(window.__TAURI_INTERNALS__?.invoke);
}

export async function seedSources(): Promise<SeedSourcesResult> {
  if (hasHostBridge()) return call<SeedSourcesResult>("indexers_seed");
  if (import.meta.env.DEV) {
    // Fixed browser-dev result: no address, no key, no network call.
    return {
      added: ["yts", "nyaasi", "thepiratebay"],
      skipped: ["eztv-api: not in Prowlarr", "animetosho: not in Prowlarr"],
    };
  }
  throw new Error("The desktop host is not available.");
}
