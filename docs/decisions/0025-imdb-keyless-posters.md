# ADR-0025: Keyless IMDb posters (supersedes artwork opt-in)

- **Status:** Accepted (2026-11-21) — human: no TMDB key available; use IMDb.
- **Supersedes:** the artwork clauses of ADR-0011 and ADR-0023 only. IMDb ratings/OMDb remain opt-in; local NFO stays the offline metadata fallback.
- **Decision:** Poster URLs come from IMDb's keyless public suggestion JSON, parsed in the Rust core with the already-present reqwest client. No API key, no Python runtime, no HTML scraping. Image bytes must come from `m.media-amazon.com/images/M/`; they are cached with the existing 2 MiB/64 MiB hashed cache. Attribution: "Posters: IMDb".
- **Risk accepted:** the suggestion endpoint is unofficial and can change or be rate-limited; failures degrade to bundled art. The human chose this over a keyed provider.
- **Preserved:** no path/key in DTOs, loopback-only serving, 64-hex key validation, 12-item cap, onError fallback.

## Evidence

- Human instruction (2026-11-21): "i cant use the tmdb thing" / "No API key" / "Try Cinemagoer/IMDb instead".
- Controller probe (2026-11-21): the keyless `https://v2.sg.media-imdb.com/suggestion/<letter>/<query>.json` endpoint returns a `d[]` array with `id` (`tt...`), `l`, `y` and `i.imageUrl`. A bare title query can return a non-title entry with id `/star-wars`, so an `id.starts_with("tt")` filter is mandatory.
- `curl.exe -I` on an `m.media-amazon.com/images/M/` poster returned HTTP 200 `image/jpeg`.
