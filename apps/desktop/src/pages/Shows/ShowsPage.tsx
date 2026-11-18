import { Link } from "react-router-dom";

import { BrowseView } from "@/components/browse/BrowseView";
import { buttonClass } from "@/components/ui/Controls";
import { seriesToMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";

export function ShowsPage() {
  const { data } = useAppState();
  if (!data) return null;

  return (
    <BrowseView
      title="TV Shows"
      description="Series, seasons and episodes from your own library. Episode lists are read from the backend, never guessed."
      items={data.shows.map(seriesToMedia)}
      emptyTitle="No series in the library yet"
      emptyBody="Add the folder that holds your series. Season and episode numbers are read from the file names, never guessed."
      emptyAction={
        <Link to="/setup" className={buttonClass("solid")} data-focusable="true">
          Add a folder
        </Link>
      }
    />
  );
}
