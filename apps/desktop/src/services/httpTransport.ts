/**
 * HTTP transport for the authenticated loopback backend (ADR-0005).
 *
 * Mirrors the backend's signature scheme: every request carries
 * `Authorization: Bearer <token>` plus an HMAC-SHA256 over
 * `timestamp || method || path || sha256(body)` keyed with the token. The
 * timestamp window is enforced server-side.
 */

export interface Transport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

function toHex(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let out = "";
  for (let i = 0; i < bytes.length; i++) {
    out += bytes[i]!.toString(16).padStart(2, "0");
  }
  return out;
}

async function sha256Hex(data: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data as unknown as BufferSource);
  return toHex(digest);
}

async function hmacSha256Hex(key: string, message: string): Promise<string> {
  const keyData = new TextEncoder().encode(key);
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    keyData as unknown as BufferSource,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    cryptoKey,
    new TextEncoder().encode(message) as unknown as BufferSource,
  );
  return toHex(signature);
}

export function createHttpTransport(baseUrl: string, token: string): Transport {
  const base = baseUrl.replace(/\/+$/, "");

  return {
    async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      const body = JSON.stringify(args ?? {});
      const timestamp = Math.floor(Date.now() / 1000).toString();
      const bodyDigest = await sha256Hex(new TextEncoder().encode(body));
      const message = `${timestamp}\nPOST\n/${command}\n${bodyDigest}`;
      const signature = await hmacSha256Hex(token, message);

      const response = await fetch(`${base}/${command}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
          "X-MediaHaven-Timestamp": timestamp,
          "X-MediaHaven-Sig": signature,
        },
        body,
      });

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
        throw parsed ?? {
          code: "internal",
          message: response.statusText || "request failed",
          retryable: false,
        };
      }

      return parsed as T;
    },
  };
}
