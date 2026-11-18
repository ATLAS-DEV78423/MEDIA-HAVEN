import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { buttonClass } from "@/components/ui/Controls";
import { toMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";

export function WatchlistPage() {
  const { data } = useAppState();
  if (!data) return null;

  const saved = data.watchlist.entries.map((entry) => toMedia(entry.media));

  return (
    <BrowseView
      title="My List"
      description="Titles saved through the backend. The list is stored in the local database and never leaves this machine."
      items={saved}
      emptyTitle="Your list is empty"
      emptyBody="Open any title and choose Add to My List. Saved titles appear here and on the home screen."
      emptyAction={
        <Link to="/movies" className={buttonClass("solid")} data-focusable="true">
          Browse movies
        </Link>
      }
    />
  );
}
