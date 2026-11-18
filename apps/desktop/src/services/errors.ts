/* Error identity and presentation shared by every renderer transport.
 * Kept dependency-free so both the host bridge and the dev client use one shape. */

export const apiBase =
  (import.meta.env.VITE_MEDIAHAVEN_API as string | undefined)?.replace(/\/$/, "") ??
  "http://127.0.0.1:8080";

export class ApiFailure extends Error {
  readonly status: number;
  readonly code: string;

  constructor(message: string, status: number, code: string) {
    super(message);
    this.name = "ApiFailure";
    this.status = status;
    this.code = code;
  }
}

export const describeError = (error: unknown): string => {
  if (error instanceof ApiFailure) return error.message;
  if (error instanceof TypeError) {
    return `The library service at ${apiBase} is not answering. Start it, then retry.`;
  }
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
};
