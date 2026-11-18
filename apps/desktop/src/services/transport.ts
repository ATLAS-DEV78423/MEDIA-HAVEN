/* The renderer transport. In the shipped desktop host every call goes through
 * the Tauri named-command bridge; the signed HTTP client is a dev-only fallback
 * reached through a DEV-guarded dynamic import. */

import { ApiFailure } from "./errors";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (
        cmd: string,
        args?: Record<string, unknown> | undefined,
      ) => Promise<unknown>;
    };
  }
}

function mapHostError(error: unknown): unknown {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof error.code === "string" &&
    typeof error.message === "string"
  ) {
    return new ApiFailure(error.message, 0, error.code);
  }
  return error;
}

export async function call<T = unknown>(
  command: string,
  args?: Record<string, unknown> | undefined,
): Promise<T> {
  const internals =
    typeof window !== "undefined" ? window.__TAURI_INTERNALS__ : undefined;
  if (internals?.invoke) {
    try {
      return (await internals.invoke(command, args)) as T;
    } catch (error) {
      throw mapHostError(error);
    }
  }
  if (import.meta.env.DEV) {
    return (await import("./devClient")).devCall(command, args) as Promise<T>;
  }
  throw new ApiFailure("The desktop host is not available.", 0, "backend_unreachable");
}
