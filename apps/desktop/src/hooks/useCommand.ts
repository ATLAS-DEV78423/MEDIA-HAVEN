import { useCallback, useEffect, useRef, useState } from "react";
import type { AppError } from "@/types/media";
import { toAppError } from "@/services/commands";

export type AsyncState<T> =
  | { readonly status: "loading" }
  | { readonly status: "ready"; readonly data: T }
  | { readonly status: "error"; readonly error: AppError };

export type UseCommandResult<T> = AsyncState<T> & {
  readonly reload: () => void;
};

/**
 * Runs a command and exposes its loading, ready and error states.
 *
 * Every screen uses this, so every screen has the same three states by
 * construction rather than by accident.
 */
export function useCommand<T>(
  run: () => Promise<T>,
  deps: readonly unknown[],
): UseCommandResult<T> {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });
  const [nonce, setNonce] = useState(0);
  const runRef = useRef(run);
  useEffect(() => {
    runRef.current = run;
  });

  useEffect(() => {
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setState({ status: "loading" });
    runRef
      .current()
      .then((data) => {
        if (!cancelled) setState({ status: "ready", data });
      })
      .catch((cause: unknown) => {
        if (!cancelled) setState({ status: "error", error: toAppError(cause) });
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, nonce]);

  const reload = useCallback(() => setNonce((n) => n + 1), []);

  return { ...state, reload };
}
