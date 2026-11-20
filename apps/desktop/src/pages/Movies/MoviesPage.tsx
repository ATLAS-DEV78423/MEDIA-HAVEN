import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { buttonClass } from "@/components/ui/Controls";
import { toMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";

export function MoviesPage() {
  const { data } = useAppState();
  if (!data) return null;

  return (
    <BrowseView
      title="Movies"
      description="Films indexed from the folders this machine is allowed to read. No rentals, no streaming queue, no account."
      items={data.movies.map((summary) => ({
        ...toMedia(summary),
        imdbRating: summary.imdbRating,
        voteCount: summary.voteCount,
        ageRating: summary.ageRating,
      }))}
      emptyTitle="No films in the library yet"
      emptyBody="Add a folder that holds your films, then scan it. MEDIA HAVEN indexes what is already on the disk and leaves the files where they are."
      emptyAction={
        <Link to="/setup" className={buttonClass("solid")} data-focusable="true">
          Add a folder
        </Link>
      }
    />
  );
}
