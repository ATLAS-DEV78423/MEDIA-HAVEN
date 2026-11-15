/**
 * Fixture library for the development mock.
 *
 * Deterministic: the same library renders identically on every load, so
 * screenshots and tests are stable.
 *
 * This contains no policy, no filesystem access and no database. It is a
 * bag of objects. See ADR-0009's revisit trigger — if this file ever grows
 * rules, the mandatory boundary has been breached.
 */

import type {
  AppSettings,
  DownloadsScreen,
  EpisodeSummary,
  LibraryRoot,
  LibraryScreen,
  MediaSummary,
  MovieDetail,
  QueueEntry,
  QueueScreen,
  SearchResults,
  SeriesDetail,
  WatchlistEntry,
  WatchlistScreen,
} from "@/types/media";
import { createArtwork } from "./artwork";

interface MovieSeed {
  readonly id: string;
  readonly title: string;
  readonly year: number;
  readonly runtime: number;
  readonly genres: readonly string[];
  readonly synopsis: string;
  readonly director: string;
  readonly cast: readonly string[];
}

const MOVIES: readonly MovieSeed[] = [
  {
    id: "m-salt-road",
    title: "The Salt Road",
    year: 2023,
    runtime: 118,
    genres: ["Drama"],
    synopsis:
      "Two cartographers walk a trade route that no longer exists, mapping a coastline that is quietly being redrawn by the sea.",
    director: "Nadia Oyelaran",
    cast: ["Imogen Vale", "Ravi Thakur", "Solveig Lund"],
  },
  {
    id: "m-nightfall",
    title: "Nightfall Protocol",
    year: 2024,
    runtime: 127,
    genres: ["Thriller", "Mystery"],
    synopsis:
      "A night-shift dispatcher starts noticing that the calls she routes out of the building match a pattern nobody designed.",
    director: "Bo Kesler",
    cast: ["Marguerite Ash", "Tobias Rune", "Pia Lindqvist"],
  },
  {
    id: "m-paper-kingdoms",
    title: "Paper Kingdoms",
    year: 2022,
    runtime: 104,
    genres: ["Adventure", "Family"],
    synopsis:
      "A twelve-year-old folds a kingdom out of discarded maps and then has to go and govern it.",
    director: "Hana Watanabe",
    cast: ["Teo Brand", "Wren Okonkwo", "Iris Callahan"],
  },
  {
    id: "m-glass-harbour",
    title: "Glass Harbour",
    year: 2021,
    runtime: 112,
    genres: ["Mystery", "Drama"],
    synopsis:
      "A marine insurer returns to the town she grew up in to settle a claim and finds the harbour keeping its own records.",
    director: "Colm Devereux",
    cast: ["Aoife Brennan", "Samuel Oduya", "Lena Rask"],
  },
  {
    id: "m-copper-orchard",
    title: "The Copper Orchard",
    year: 2024,
    runtime: 96,
    genres: ["Drama"],
    synopsis:
      "Three siblings inherit an orchard that has not been harvested in a decade, and a debt that grows a little every season.",
    director: "Aurelio Marchetti",
    cast: ["Nadia Petrossian", "Femi Adeyemi", "Joanna Stroud"],
  },
  {
    id: "m-signal-lost",
    title: "Signal Lost",
    year: 2023,
    runtime: 133,
    genres: ["Sci-Fi", "Thriller"],
    synopsis:
      "The relay station has been answering for eleven years. The technician who maintains it is not sure that is a good thing.",
    director: "Priya Raghunathan",
    cast: ["Dmitri Vasiliev", "Anouk Ferrand", "Cora Whitfield"],
  },
  {
    id: "m-lantern-season",
    title: "Lantern Season",
    year: 2020,
    runtime: 108,
    genres: ["Drama", "Romance"],
    synopsis:
      "Every autumn the fishing boats bring back a lantern that has already been lost at sea, and every autumn someone takes it.",
    director: "Émile Larouche",
    cast: ["Sofia Marchetti", "Ibrahim Toure", "Wren Okonkwo"],
  },
  {
    id: "m-blackwater-ridge",
    title: "Blackwater Ridge",
    year: 2024,
    runtime: 141,
    genres: ["Western"],
    synopsis:
      "A land surveyor is hired to mark a ridge for a railway, and discovers the company has already built the line.",
    director: "Harlan Voss",
    cast: ["Clem Ashworth", "Rina Delacroix", "August Peiro"],
  },
  {
    id: "m-quiet-machine",
    title: "The Quiet Machine",
    year: 2022,
    runtime: 99,
    genres: ["Sci-Fi"],
    synopsis:
      "An engineer builds a device designed to prove that a person is not lying, and then has to live with what it proves.",
    director: "Ilaria Bonetti",
    cast: ["Yusuf Demir", "Greta Sandvik", "Louis Amankwah"],
  },
  {
    id: "m-ember-court",
    title: "Ember Court",
    year: 2025,
    runtime: 152,
    genres: ["Fantasy", "Adventure"],
    synopsis:
      "The court convenes once a century to decide which of the old fires may be lit again. This year, two families arrive with the same claim.",
    director: "Rhiannon Price",
    cast: ["Caleb Nwosu", "Elke Sommer", "Tamsin Blake"],
  },
  {
    id: "m-northbound",
    title: "Northbound",
    year: 2021,
    runtime: 121,
    genres: ["Adventure", "Drama"],
    synopsis:
      "A freight crew takes the winter route when the pass closes, and finds a road that the maps disagree about.",
    director: "Sanna Vikander",
    cast: ["Kofi Mensah", "Astrid Holm", "Ren Takahashi"],
  },
  {
    id: "m-halfway",
    title: "Halfway to Nowhere",
    year: 2023,
    runtime: 94,
    genres: ["Comedy"],
    synopsis:
      "A man misses the last train home every single night for reasons that accumulate.",
    director: "Dolores Fenn",
    cast: ["Milo Kranz", "Bibi Nwachukwu", "Otto Lindgren"],
  },
  {
    id: "m-cartographer",
    title: "The Cartographer",
    year: 2019,
    runtime: 116,
    genres: ["Drama", "History"],
    synopsis:
      "A mapmaker is commissioned to chart a border, and quietly redraws it to keep two families on the same side.",
    director: "Anselm Duclos",
    cast: ["Greta Sandvik", "Hamid Ferhat", "Nell Ashby"],
  },
  {
    id: "m-deep-field",
    title: "Deep Field",
    year: 2024,
    runtime: 129,
    genres: ["Sci-Fi", "Mystery"],
    synopsis:
      "The observatory receives a signal older than the telescope, and the team that answers it stops being a team.",
    director: "Yuki Aramaki",
    cast: ["Sigrid Halvorsen", "Emeka Nwankwo", "Lior Ben-Ami"],
  },
  {
    id: "m-low-tide",
    title: "Low Tide",
    year: 2022,
    runtime: 105,
    genres: ["Thriller", "Crime"],
    synopsis:
      "For six hours a day the causeway is passable, and someone is using it to move things nobody wants found.",
    director: "Rafael Sandoval",
    cast: ["Imogen Vale", "Patrick Nkemelu", "Delia Marsh"],
  },
  {
    id: "m-alder-street",
    title: "Alder Street",
    year: 2020,
    runtime: 97,
    genres: ["Comedy", "Drama"],
    synopsis:
      "Four neighbours on a terraced street each decide to renovate, and the street decides to fight back.",
    director: "Nils Aaberg",
    cast: ["Bea Cartwright", "Zaid Haddad", "Marta Wisniewska"],
  },
  {
    id: "m-long-return",
    title: "The Long Return",
    year: 2025,
    runtime: 138,
    genres: ["Western", "Drama"],
    synopsis:
      "A rider comes back to a ranch that no longer has a name, to find out who has been working the land.",
    director: "Harlan Voss",
    cast: ["August Peiro", "Sana Qureshi", "Louis Amankwah"],
  },
  {
    id: "m-winterlight",
    title: "Winterlight",
    year: 2021,
    runtime: 111,
    genres: ["Fantasy", "Romance"],
    synopsis:
      "In a town where the sun arrives late, a lamp-lighter falls in love with the person who keeps the clock.",
    director: "Marit Solheim",
    cast: ["Elke Sommer", "Kofi Mensah", "Nora Bellamy"],
  },
  {
    id: "m-static-bloom",
    title: "Static Bloom",
    year: 2023,
    runtime: 102,
    genres: ["Mystery", "Sci-Fi"],
    synopsis:
      "A botanist's greenhouse starts reproducing plants that have not been catalogued, including one she has never seen.",
    director: "Priya Raghunathan",
    cast: ["Tamsin Blake", "Cora Whitfield", "Yusuf Demir"],
  },
  {
    id: "m-understudy",
    title: "The Understudy",
    year: 2024,
    runtime: 107,
    genres: ["Drama"],
    synopsis:
      "A repertory actor waits in the wings for a part that keeps going to somebody else.",
    director: "Dolores Fenn",
    cast: ["Greta Sandvik", "Bibi Nwachukwu", "Pierre Lacroix"],
  },
];

interface SeriesSeed {
  readonly id: string;
  readonly title: string;
  readonly year: number;
  readonly genres: readonly string[];
  readonly synopsis: string;
  readonly seasons: readonly {
    readonly number: number;
    readonly title: string;
    readonly episodeTitles: readonly string[];
  }[];
}

const SERIES: readonly SeriesSeed[] = [
  {
    id: "s-meridian",
    title: "Meridian",
    year: 2023,
    genres: ["Sci-Fi", "Drama"],
    synopsis:
      "A weather station on the last inhabited ridge keeps sending reports for a town that has no inhabitants.",
    seasons: [
      {
        number: 1,
        title: "Pressure",
        episodeTitles: [
          "The Instrument Fails",
          "Alpine Signal",
          "Snowline",
          "Contour Map",
          "The Long Cold",
        ],
      },
      {
        number: 2,
        title: "Gradient",
        episodeTitles: [
          "Meridian Drift",
          "Cumulative Error",
          "Whiteout",
          "The Second Ridge",
        ],
      },
      {
        number: 3,
        title: "Forecast",
        episodeTitles: ["Clear Air", "The Reading", "Terminal Conditions"],
      },
    ],
  },
  {
    id: "s-hollow-coast",
    title: "The Hollow Coast",
    year: 2022,
    genres: ["Mystery", "Crime"],
    synopsis:
      "A detective returns to the fishing villages she left behind and finds the same case, still open, still hers.",
    seasons: [
      {
        number: 1,
        title: "Shoreline",
        episodeTitles: [
          "The Lifeboat",
          "Low Water",
          "Six Names",
          "Bell Harbour",
          "What the Tide Left",
        ],
      },
      {
        number: 2,
        title: "Spring Tide",
        episodeTitles: ["Neap", "The Second Boat", "Salt in the Ledger"],
      },
    ],
  },
  {
    id: "s-foxglove",
    title: "Foxglove",
    year: 2024,
    genres: ["Comedy", "Drama"],
    synopsis:
      "A village horticultural society takes a formal complaint against a member who grows the wrong things.",
    seasons: [
      {
        number: 1,
        title: "Propagation",
        episodeTitles: [
          "Seed Order",
          "The Committee",
          "Hardiness Zone",
          "Slip and Cut",
          "Winter Structure",
        ],
      },
      {
        number: 2,
        title: "Transplant",
        episodeTitles: ["Bare Root", "The Annual Show", "Hardy Perennial"],
      },
    ],
  },
  {
    id: "s-redshift",
    title: "Redshift",
    year: 2021,
    genres: ["Sci-Fi", "Thriller"],
    synopsis:
      "Every message from Earth arrives eight years late, which is just long enough to change what you would have done.",
    seasons: [
      {
        number: 1,
        title: "Departure",
        episodeTitles: [
          "Eight Years",
          "Echo",
          "The Delay",
          "Reception",
          "Return Window",
        ],
      },
      {
        number: 2,
        title: "Arrival",
        episodeTitles: ["The Reply", "Correction", "Parallax"],
      },
      {
        number: 3,
        title: "Correspondence",
        episodeTitles: ["Quiet Zone", "The Last Letter", "Closing Distance"],
      },
    ],
  },
  {
    id: "s-cold-harbour",
    title: "Cold Harbour",
    year: 2025,
    genres: ["Drama"],
    synopsis:
      "A freight port runs on hand-written ledgers, and the new management wants to replace them with software.",
    seasons: [
      {
        number: 1,
        title: "Manifest",
        episodeTitles: [
          "Gate 4",
          "The Weight Clause",
          "Short Weight",
          "Customs House",
          "Last Call",
        ],
      },
    ],
  },
  {
    id: "s-ninth-ward",
    title: "The Ninth Ward",
    year: 2022,
    genres: ["Crime", "Thriller"],
    synopsis:
      "A neighbourhood watch programme gets serious enough that its members start watching each other.",
    seasons: [
      {
        number: 1,
        title: "Rounds",
        episodeTitles: [
          "Foot Patrol",
          "Curfew",
          "The Parking Committee",
          "Neighbourhood Improvement",
        ],
      },
      {
        number: 2,
        title: "Boundary",
        episodeTitles: ["Ward Line", "The Committee", "Quorum"],
      },
    ],
  },
];

const GENRE_ART = ["Drama", "Thriller", "Sci-Fi", "Comedy", "Western", "Mystery", "Fantasy", "Adventure"];

function posterAndBackdrop(title: string, subtitle?: string) {
  return [
    createArtwork(title, "poster", subtitle),
    createArtwork(title, "backdrop", subtitle),
  ] as const;
}

/* ------------------------------------------------------------------ *
 * Derived media
 * ------------------------------------------------------------------ */

function movieSummary(seed: MovieSeed): MediaSummary {
  return {
    id: seed.id,
    kind: "movie",
    title: seed.title,
    year: seed.year,
    synopsis: seed.synopsis,
    runtimeMinutes: seed.runtime,
    genres: seed.genres,
    artwork: posterAndBackdrop(seed.title),
    availability: { state: "available" },
    progress: null,
  };
}

function episodeSummaries(series: SeriesSeed): EpisodeSummary[] {
  const out: EpisodeSummary[] = [];
  for (const season of series.seasons) {
    season.episodeTitles.forEach((title, index) => {
      out.push({
        id: `${series.id}-s${season.number}e${index + 1}`,
        seriesId: series.id,
        seriesTitle: series.title,
        seasonNumber: season.number,
        episodeNumber: index + 1,
        title,
        runtimeMinutes: 42 + ((index * 7 + season.number * 3) % 19),
        artwork: posterAndBackdrop(`${series.title} — ${title}`, series.title),
        availability: { state: "available" },
        progress: null,
        nextUp: false,
      });
    });
  }
  return out;
}

export const MOCK_MOVIES: readonly MediaSummary[] = MOVIES.map(movieSummary);

export const MOCK_MOVIE_DETAILS: ReadonlyMap<string, MovieDetail> = new Map(
  MOVIES.map((seed) => {
    const summary = movieSummary(seed);
    const detail: MovieDetail = {
      ...summary,
      kind: "movie",
      director: seed.director,
      cast: seed.cast,
    };
    return [seed.id, detail];
  }),
);

export const MOCK_SERIES: readonly SeriesDetail[] = SERIES.map((series) => ({
  id: series.id,
  kind: "series",
  title: series.title,
  year: series.year,
  synopsis: series.synopsis,
  genres: series.genres,
  artwork: posterAndBackdrop(series.title),
  seasons: series.seasons.map((season) => ({
    number: season.number,
    title: season.title,
    episodeCount: season.episodeTitles.length,
  })),
}));

export const MOCK_EPISODES: ReadonlyMap<string, EpisodeSummary[]> = new Map(
  SERIES.map((series) => [series.id, episodeSummaries(series)]),
);

/* ------------------------------------------------------------------ *
 * Playback state — a believable in-progress session
 * ------------------------------------------------------------------ */

const CONTINUE_IDS = ["m-deep-field", "s-redshift", "m-low-tide", "m-glass-harbour"];

export const MOCK_CONTINUE: readonly MediaSummary[] = CONTINUE_IDS.map((id, index) => {
  const movie = MOVIES.find((m) => m.id === id);
  const series = SERIES.find((s) => s.id === id);
  const base = movie
    ? movieSummary(movie)
    : {
        id: series!.id,
        kind: "series" as const,
        title: series!.title,
        year: series!.year,
        synopsis: series!.synopsis,
        runtimeMinutes: null,
        genres: series!.genres,
        artwork: posterAndBackdrop(series!.title),
        availability: { state: "available" as const },
        progress: null,
      };
  const durationSeconds = (base.runtimeMinutes ?? 52) * 60;
  const positionSeconds = Math.round(durationSeconds * (0.18 + index * 0.14));
  return {
    ...base,
    progress: {
      positionSeconds,
      durationSeconds,
      updatedAt: `2026-11-1${4 - index}T21:0${index}:00Z`,
      completed: false,
    },
  };
});

export const MOCK_RECENTLY_ADDED_MOVIES: readonly MediaSummary[] = [
  ...MOCK_MOVIES.slice(14, 20),
  ...MOCK_MOVIES.slice(2, 8),
];

export const MOCK_RECENTLY_ADDED_SHOWS: readonly MediaSummary[] = MOCK_SERIES.map(
  (series) => ({
    id: series.id,
    kind: "series" as const,
    title: series.title,
    year: series.year,
    synopsis: series.synopsis,
    runtimeMinutes: null,
    genres: series.genres,
    artwork: series.artwork,
    availability: { state: "available" as const },
    progress: null,
  }),
);

export const MOCK_RECOMMENDED: readonly MediaSummary[] = [
  ...MOCK_MOVIES.slice(0, 6),
  ...MOCK_RECENTLY_ADDED_SHOWS.slice(0, 3),
];

export const MOCK_RECENTLY_PLAYED: readonly MediaSummary[] = MOCK_MOVIES.slice(6, 12);

export const MOCK_GENRES: readonly string[] = GENRE_ART;

/* ------------------------------------------------------------------ *
 * Watchlist and queue
 * ------------------------------------------------------------------ */

export const MOCK_WATCHLIST_ENTRIES: readonly WatchlistEntry[] = [
  ...MOCK_MOVIES.slice(9, 13),
  ...MOCK_RECENTLY_ADDED_SHOWS.slice(1, 3),
].map((media, index) => ({
  id: `wl-${media.id}`,
  media,
  position: index,
  addedAt: `2026-11-${String(10 - index).padStart(2, "0")}T18:00:00Z`,
}));

export const MOCK_QUEUE_ENTRIES: readonly QueueEntry[] = MOCK_CONTINUE.map(
  (media, index) => ({
    id: `q-${media.id}`,
    media,
    position: index,
    state: index === 0 ? "active" : "pending",
  }),
);

/* ------------------------------------------------------------------ *
 * Library roots and settings
 * ------------------------------------------------------------------ */

export const MOCK_ROOTS: readonly LibraryRoot[] = [
  {
    id: "root-1",
    displayName: "Films",
    itemCount: 428,
    status: "ok",
    lastScanAt: "2026-11-15T08:12:00Z",
    errorMessage: null,
  },
  {
    id: "root-2",
    displayName: "Series",
    itemCount: 96,
    status: "ok",
    lastScanAt: "2026-11-15T08:14:00Z",
    errorMessage: null,
  },
  {
    id: "root-3",
    displayName: "Archive (USB)",
    itemCount: 1_204,
    status: "offline",
    lastScanAt: "2026-11-02T19:40:00Z",
    errorMessage:
      "This folder is offline or disconnected. Its media is still in your library.",
  },
];

export const MOCK_SETTINGS: AppSettings = {
  libraryRoots: MOCK_ROOTS,
  playback: {
    preferredLanguage: "en",
    subtitleSize: "medium",
    autoplayNext: false,
  },
  storage: {
    rootDisplayName: "Managed media",
    limitBytes: 250 * 1024 * 1024 * 1024,
  },
  optionalServices: {
    downloadsEnabled: false,
    metadataProvider: null,
  },
  diagnostics: {
    runtimeStatus: { state: "ready" },
    backendStatus: { state: "ready" },
  },
};

export const MOCK_LIBRARY_SCREEN: LibraryScreen = {
  roots: MOCK_ROOTS,
  counts: { movies: 428, shows: 96, episodes: 412 },
};

export const MOCK_QUEUE_SCREEN: QueueScreen = { entries: MOCK_QUEUE_ENTRIES };

export const MOCK_WATCHLIST_SCREEN: WatchlistScreen = {
  entries: MOCK_WATCHLIST_ENTRIES,
};

export const MOCK_DOWNLOADS_SCREEN: DownloadsScreen = {
  enabled: false,
  tasks: [],
  storageUsedBytes: 0,
  storageLimitBytes: null,
};

export function searchFixtures(text: string): SearchResults {
  const needle = text.trim().toLowerCase();
  const all: MediaSummary[] = [
    ...MOCK_MOVIES,
    ...MOCK_RECENTLY_ADDED_SHOWS,
  ];
  const items = needle
    ? all.filter(
        (media) =>
          media.title.toLowerCase().includes(needle) ||
          (media.synopsis ?? "").toLowerCase().includes(needle) ||
          media.genres.some((g) => g.toLowerCase().includes(needle)),
      )
    : all;
  return {
    query: { text, kinds: [], genres: [] },
    items: items.slice(0, 60),
    total: items.length,
    truncated: items.length > 60,
  };
}
