/**
 * The transport seam.
 *
 * Exactly one file decides where commands go. Today that is the development
 * mock; later it is the Tauri host, which forwards to the containerised
 * backend over the authenticated loopback API (ADR-0005, ADR-0009).
 *
 * Nothing above this file knows the difference.
 */

import type { Commands } from "@/types/media";

export interface Transport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

/**
 * The Tauri host transport.
 *
 * It talks to `window.__TAURI_INTERNALS__.invoke`, which is the IPC entry
 * point Tauri's own `@tauri-apps/api` package wraps. Using the global
 * directly keeps the frontend free of a runtime dependency on the Rust
 * shell, which does not exist yet — see roadmap P1-08.
 */
export function createHostTransport(): Transport {
  return {
    async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__?: {
            invoke: (cmd: string, args?: unknown, options?: unknown) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;

      if (!internals) {
        throw new Error(
          `Command "${command}" was sent to the desktop host, but the host is not available.`,
        );
      }

      return (await internals.invoke(command, args ?? {})) as T;
    },
  };
}

export type TransportKind = "mock" | "host";

/**
 * Chooses the transport.
 *
 * The mock is imported dynamically and only in development, so the
 * production bundle never contains it. `npm run check:tokens` asserts the
 * bundle contains no mock code.
 */
export async function createTransport(): Promise<{
  transport: Transport;
  kind: TransportKind;
}> {
  if (import.meta.env.DEV) {
    const { createMockTransport } = await import("./mock/handler");
    return { transport: createMockTransport(), kind: "mock" };
  }
  return { transport: createHostTransport(), kind: "host" };
}

export type { Commands };
