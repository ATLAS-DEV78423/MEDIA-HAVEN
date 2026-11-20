# Privacy and data flow

MEDIA HAVEN is local-first. The interface talks to a local service over a
signed loopback connection. There is no account, no subscription and no
telemetry. File paths stay host-internal and never appear in a renderer DTO.

## Metadata sources

Local NFO sidecars are read beside the media file. TMDB remains the primary
network provider when the operator opts in (ADR-0011); IMDb data added on
2026-11-21 is **additive and opt-in** (ADR-0022, Proposed). It does not replace
TMDB or local NFO, and local behaviour is unchanged when it is off.

## IMDb datasets (ratings, vote counts, listings)

- The operator supplies the dataset files (`title.basics.tsv` and
  `title.ratings.tsv`). Nothing is bundled, downloaded, indexed or scraped by
  the app.
- Reading and parsing these files happens on this machine. **No request is
  made and nothing is sent** to obtain a rating from the datasets.
- If enabled later, the only lookup key is an IMDb id (a `tt` + digits value).
  No file path, folder name or library content is included.

## OMDb (age/certification ratings)

- Off by default. When enabled by the operator, the only title-related value
  sent to OMDb is an **IMDb id**, together with an API key the operator supplies
  from the local secret store (`MEDIAHAVEN_OMDB_API_KEY`).
- The key is never read from or written to the repository, logs, settings
  document or any renderer DTO. A missing key yields `Unavailable`; no rating is
  fabricated.
- Live OMDb calls require an explicit operator opt-in and key. None were made
  by the implementation tasks; that path remains untested here.

## Opt-in controls and current limitation

Both `imdb.enabled` and `omdb.enabled` default to `false`. The Settings screen
shows "IMDb ratings & listings" and "OMDb age ratings" as **disabled switches**
labelled "Not available": there is no backend command wired to flip them in
this build, so both stay off. The switches remain visible so the data flow is
honest. Enabling them requires backend/settings wiring owned by a later task.

## Caching and attribution

- Age ratings are cached locally with a 30-day TTL, a byte cap and oldest-first
  eviction (`imdb_cache` table). Only sanitised certification strings, an id, a
  timestamp and a source label are stored — no key, path or token.
- Failures degrade to local data and never break a view. No rating appears
  unless it was actually read from the operator's dataset or cache; the UI
  renders nothing when the value is absent.
- Wherever IMDb or OMDb data is displayed, the app shows:
  **"Ratings and age ratings: IMDb / OMDb"**.

## What is never sent

File paths, folder names, directory listings, media contents, the local
database, and the OMDb key (except as authentication to OMDb once the operator
has opted in). No third-party analytics or tracker ships with the app.
