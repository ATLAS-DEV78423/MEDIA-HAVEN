/* Renderer-side playback bridge (ADR-0015).
 *
 * In a browser dev session there is no Tauri host: every function is a no-op
 * and the simulated clock remains the fallback. In the host, state comes from
 * `player://state` events and controls go through the named `player_control`
 * command. No path or token is carried on either channel — only display
 * numbers and the media id the renderer already owns. */

import { call } from "./transport";

export interface PlayerStateEvent {
  timePos: number;
  pause: boolean;
  volume: number;
  mediaId: string;
}

export type PlayerControlAction = "play" | "pause" | "seek" | "volume";

const PLAYER_STATE_EVENT = "player://state";

/* `@tauri-apps/api` is not a project dependency. Reuse the internal bridge
 * that `transport.ts` already declares, and stay inert when it is absent. */
interface Internals {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback?: (callback: (payload: unknown) => void) => number;
  unregisterCallback?: (id: number) => void;
}

function hostInternals(): Internals | undefined {
  if (typeof window === "undefined") return undefined;
  return window.__TAURI_INTERNALS__ as Internals | undefined;
}

export function isTauriHost(): boolean {
  return typeof hostInternals()?.invoke === "function";
}

/** Subscribe to engine state. Returns a teardown function; no-op in a browser. */
export function listenPlayerState(cb: (state: PlayerStateEvent) => void): () => void {
  const host = hostInternals();
  if (!host?.invoke || !host.transformCallback) return () => {};

  let stopped = false;
  let listenId: number | undefined;
  const handler = host.transformCallback((payload: unknown) => {
    if (!stopped && payload && typeof payload === "object") {
      cb(payload as PlayerStateEvent);
    }
  });

  void host
    .invoke("plugin:event|listen", { event: PLAYER_STATE_EVENT, handler })
    .then((id) => {
      if (typeof id !== "number") return;
      if (stopped) {
        void host.invoke("plugin:event|unlisten", { id });
        host.unregisterCallback?.(id);
      } else {
        listenId = id;
      }
    })
    .catch(() => {});

  return () => {
    stopped = true;
    if (listenId === undefined) return;
    void host.invoke("plugin:event|unlisten", { id: listenId }).catch(() => {});
    host.unregisterCallback?.(listenId);
  };
}

/** Drive the local engine through a named command; no-op outside the host. */
export function playerControl(action: PlayerControlAction, value?: number): Promise<void> {
  if (!isTauriHost()) return Promise.resolve();
  return call<void>("player_control", { action, value: value ?? null });
}
