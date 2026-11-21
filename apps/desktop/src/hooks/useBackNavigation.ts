import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

/* TV-remote "back": Escape returns to the previous screen. Dialogs and the
   player handle their own Escape and call preventDefault(), so this handler
   deliberately does nothing when the event was already consumed. */

export function useBackNavigation(): void {
  const navigate = useNavigate();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      // The player owns Escape (it navigates back itself).
      if (document.documentElement.dataset.player === "active") return;
      // An open modal owns Escape (profiles dialog, AddSource dialog).
      const modal = document.querySelector(
        '[role="dialog"][aria-modal="true"]',
      );
      if (modal && !event.defaultPrevented) {
        // Let the dialog's own handler run; if none does, fall through on the
        // next tick is not possible, so we navigate only when no modal exists.
        return;
      }
      event.preventDefault();
      // react-router keeps the history index in history.state.idx when using a
      // browser router; with hash routing this may be undefined, in which case
      // go Home instead of navigating nowhere.
      const idx = (window.history.state as { idx?: number } | null)?.idx;
      if (typeof idx === "number" && idx > 0) {
        navigate(-1);
      } else {
        navigate("/", { replace: true });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate]);
}
