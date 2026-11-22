/* Startup lives in the Tauri host: it starts the backend before the renderer
 * asks the library for data. In browser dev there is no host to start, so the
 * facade returns a fixed status and never invokes a command. */

import { call } from "./transport";

export async function startRuntime(): Promise<string> {
  if (import.meta.env.DEV) return "dev";
  return call<string>("runtime_start", {});
}
