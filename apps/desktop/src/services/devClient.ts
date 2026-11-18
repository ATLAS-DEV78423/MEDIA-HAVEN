/* Dev-only signed HTTP client. Loaded exclusively through the DEV-guarded
 * dynamic import in transport.ts, so this token and this fetch never ship in
 * the production bundle. */

import { apiBase, ApiFailure } from "./errors";

const DEV_TOKEN = "dev-only-not-for-production";

const token =
  (import.meta.env.VITE_MEDIAHAVEN_TOKEN as string | undefined) ??
  (import.meta.env.DEV ? DEV_TOKEN : "");

const encoder = new TextEncoder();

function toHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function sign(
  timestamp: string,
  method: string,
  path: string,
  body: string,
): Promise<string> {
  if (!token) {
    throw new ApiFailure(
      "The install token is missing. The desktop host must provide it.",
      0,
      "missing_token",
    );
  }
  const digest = toHex(await crypto.subtle.digest("SHA-256", encoder.encode(body)));
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(token),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const mac = await crypto.subtle.sign(
    "HMAC",
    key,
    encoder.encode(`${timestamp}\n${method}\n${path}\n${digest}`),
  );
  return toHex(mac);
}

async function request(
  method: "GET" | "POST",
  path: string,
  payload?: unknown,
): Promise<unknown> {
  const body = method === "GET" ? "" : JSON.stringify(payload ?? {});
  const timestamp = String(Math.floor(Date.now() / 1000));
  const signature = await sign(timestamp, method, path, body);

  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, {
      method,
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
        "X-MediaHaven-Timestamp": timestamp,
        "X-MediaHaven-Sig": signature,
      },
      ...(method === "GET" ? {} : { body }),
    });
  } catch {
    throw new ApiFailure(
      `The library service at ${apiBase} is not answering. Start it, then retry.`,
      0,
      "unreachable",
    );
  }

  const text = await response.text();
  let parsed: unknown = null;
  if (text) {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = null;
    }
  }

  if (!response.ok) {
    const record = (parsed ?? {}) as { code?: string; message?: string };
    throw new ApiFailure(
      record.message ?? `The library service rejected the request (${response.status}).`,
      response.status,
      record.code ?? "unknown",
    );
  }

  return parsed;
}

export async function devCall(
  command: string,
  args?: Record<string, unknown> | undefined,
): Promise<unknown> {
  return request("POST", `/${command}`, args);
}
