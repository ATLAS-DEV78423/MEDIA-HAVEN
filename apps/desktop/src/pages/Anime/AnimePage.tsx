import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { buttonClass } from "@/components/ui/Controls";
import { seriesToMedia, toMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";

export function AnimePage() {
  const { data } = useAppState();
  if (!data) return null;

  const anime = [
    ...data.movies.map(toMedia),
    ...data.shows.map(seriesToMedia),
  ].filter((media) =>
    media.genres.some((genre) => genre.toLowerCase() === "anime"),
  );

  return (
    <BrowseView
      title="Anime"
      description="Anything tagged Anime in the library's own metadata. Tags come from the files and the backend, never from a catalogue service."
      items={anime}
      emptyTitle="No anime tagged yet"
      emptyBody="Titles appear here when their indexed genres include Anime. Add a folder or tag titles in the library metadata."
      emptyAction={
        <Link to="/movies" className={buttonClass("solid")} data-focusable="true">
          Browse everything
        </Link>
      }
    />
  );
}
