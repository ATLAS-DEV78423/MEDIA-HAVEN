# MEDIA_HAVEN — Complete Session Handoff

**Date:** 2026-11-22
**Branch:** `tmdb-artwork-and-exe-guard` (in worktree `C:\Users\Stanley\.local\share\opencode\worktree\ea5b46\tmdb-artwork`)
**Head at handoff:** the work is committed on that branch; `master` has not been changed and nothing has been pushed.

---

## 1. Where we stopped

Three things finished in this session:

1. **Executable-download guard** (your security request).
2. **Keyless IMDb posters** (no API key, no Python, no opt-in) — verified end to end through the HTTP API.
3. **This handoff** — plus an assessment of the Miauflix repo you linked.

The immediately preceding action was an end-to-end check: I started the updated backend on `127.0.0.1:8081` with seeded data, called `POST /library_home` with a signed request, got 10 items of which 7 had `/artwork/<64-hex>` URLs, then fetched one with `curl` — **HTTP 200, `image/jpeg`, 53,600 bytes, `x-content-type-options: nosniff`, valid JPEG magic `FF-D8-FF-E0`**. The test server was stopped afterwards. A background task then ended with a nonzero code because I killed it; that was intentional, not a failure of the app.

**Not done:** no UI screenshot in a real browser/WebView, no full-library poster seed, no live qB download, no merge into `master`, no push, no checkpoint sign-off.

---

## 2. What has been built (full history)

### Core product
- Local-first media appliance: Tauri desktop frontend + Rust `mediahaven-core` HTTP backend + Docker `probe` container for media inspection + qBittorrent in an isolated `acquire` network.
- Library scanning, SQLite persistence, profiles, watchlist, queue, playback progress, settings, diagnostics.
- Full keyboard navigation: global Escape-to-back, ArrowUp/Down exits single-line text inputs, spatial focus; 4 unit tests.
- Unsigned Windows NSIS installer at `target/release/bundle/nsis/MEDIA_HAVEN_0.1.0_x64-setup.exe` with `SHA256SUMS.txt` and `THIRD_PARTY_NOTICES.md`.

### Torrent acquisition (earlier work)
- ADR-0017/0018/0019 accepted: authorized-media acquisition through qBittorrent, no bundled indexers, app-managed cleanup.
- Named host commands, WebUI client, Downloads/Settings UI, rolling buffer, coordinated cleanup.
- **Security gates:** ffprobe inside a networkless shell-less probe container with a real read-only `/downloads` bind; Windows Defender argv-only scan; both gates on `playback_start` and `detail()`, fail-closed parsers.
- **R-01 source mapping:** per-item operator-supplied source (`item_sources`), `source_set` chain, download-remove persistence, UI status.
- **IMDb metadata (separate, additive):** TSV parser/cache, OMDb seam, display-only fields with attribution. OMDb HTTP transport is still a future seam needing a key and ADR.

### This session's new work
- **Executable-download guard (ADR-0024):** torrents are added with `paused=true`; declared file names are checked for 29 Windows-executable extensions (case-insensitive, double extensions, both separators); a bad/empty/unreadable list causes `remove(hash, false)` and `AcquireError::Rejected`, never `resume`. `AcquireError::Rejected` is a fixed path-free string. The ffprobe/Defender gates remain the second layer.
- **Keyless IMDb posters (ADR-0025):** `ImdbArtworkProvider` queries `https://v2.sg.media-imdb.com/suggestion/<letter>/<query>.json`, requires a `tt...` id with title/year match, accepts only `https://m.media-amazon.com/images/M/` images, caches bytes with the existing hashed cache, normalizes URLs to IMDb's `_QL75_UX500_` small variant so they fit the 2 MiB cap, and has a bounded 30-day title/year→URL cache. TMDB wiring was removed from `core`. Attribution is `Posters: IMDb`.
- The CSP gained exactly one origin (`http://127.0.0.1:8080`) in `img-src`; frontend `artFromDto` prefers backend URLs with strict `^\/artwork\/[0-9a-f]{64}$` validation and a guarded `onError` fallback to bundled art.

---

## 3. Decisions and ADRs

| ADR | Decision |
|---|---|
| 0011 | TMDB primary, local NFO fallback, network opt-in — **artwork clauses now superseded by ADR-0025** |
| 0016 | Blanket acceptance (brand, Windows 11 x64, offline install, unsigned installer) |
| 0017–0019 | Acquisition conversion, qBittorrent boundary, app-managed cleanup |
| 0021 | R-01 per-item source mapping |
| 0022 | IMDb ratings (additive, opt-in) — still Proposed |
| 0023 | TMDB artwork transport / hashed route — **artwork opt-in superseded by ADR-0025** |
| 0024 | Reject executable-bearing torrents before download — Accepted |
| 0025 | Keyless IMDb posters accepted; unofficial endpoint risk accepted — Accepted |

Controller rulings made on your behalf this session (all were recorded in SDD ledgers):
- Commits were permitted by "you have my permission to make the changes"; push/merge were not.
- `reqwest` with rustls (never native-tls/OpenSSL); ADR-0023 recorded as Accepted, not Proposed.
- Shared `fill_poster` helper instead of duplicated loops; route test uses `/artwork/not-a-hash`.
- Attribution gated in Details, unconditional in Hero; CSS uses tokens only.
- Fix-wave choices after final review: 12-item per-batch enrichment cap, guarded `onError` fallback, exact `magnet_uri` match in qB add fallback, `nosniff`, logged removal failures, bounded URL cache (512 entries, clear-on-cap), IMDb small-image variant with the 2 MiB cap untouched.
- **No Python/Cinemagoer runtime** — you chose the IMDb route, I kept the implementation in the existing Rust core. Same keyless data, no new runtime.
- **Rejected, on purpose:** bundling Miauflix's site list (see §6).

---

## 4. Tests and verification actually conducted

| Check | Result |
|---|---|
| `cargo test -p media-metadata` (final) | 58 passed / 0 failed |
| `cargo test -p mediahaven-core` | 25 passed / 0 failed |
| `cargo test -p media-acquisition` | 64 passed / 0 failed |
| `cargo test -p mediahaven-contract` | 8 passed / 0 failed (24-command route parity green) |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p media-metadata -p mediahaven-core -p media-acquisition --all-targets -- -D warnings` | PASS |
| Frontend `typecheck / lint / test / check:tokens / build / check:bundle` | PASS — 36 Vitest tests, 77 token files |
| `git diff --check` | PASS |
| Live IMDb suggestion probe | Arrival, Dune, Blade Runner 2049 all returned keyless `tt...` matches + `m.media-amazon.com` URLs |
| Live poster fetch via provider | Arrival 53,600 B, Dune 43,766 B, Blade Runner 2049 60,809 B — all `image/jpeg` |
| End-to-end backend probe | `/library_home` returned 7/10 items with artwork URLs; `GET /artwork/<hash>` → 200, `image/jpeg`, 53,600 B, `nosniff` |

**NOT RUN:** live qB rejection of a real executable-bearing torrent; a real browser/WebView screenshot; full-library seed; second-machine LAN non-reachability; installer on a clean machine; any checkpoint sign-off.

Review history: every task had an implementer and a fresh reviewer; the final whole-branch reviews found 7 findings (TMDB chain) and 5 + 1 runtime findings (IMDb chain), all fixed in single fix waves and scoped-re-reviewed clean.

---

## 5. The CSV plan (original idea) — how it would be made

This was **never executed**. It remains valid if you choose the local-catalog route instead of the Miauflix-style provider.

**Rule:** the scraper runs **once, outside the app**. The app never contacts a website and never bundles site definitions.

1. **You supply the URL** of the site whose entries you are authorized to use.
2. I use your already-verified Firecrawl key (`/v2/scrape` or `/v2/crawl`) to extract entries into a plain table. One page first, sample shown to you, format confirmed before doing more.
3. **Columns (exact):**
   `title, year, kind(movie|series), season, episode, release_name, magnet, source_url, captured_at`
   - `magnet` is stored defanged for human reading (`magnet:?xt=...` shown as `magnet[://]...`) and re-fanged only at use time.
   - No file bytes, no comments, no credentials, no site passwords, no tracker invite data.
4. **Normalization:** Unicode NFKD, lowercase, strip brackets/release tags, collapse whitespace; parse `S01E02` into season/episode; keep the original `release_name` for traceability.
5. **Dedupe:** by info-hash from `magnet:?xt=urn:btih:`; keep the first row, record duplicates in a separate rejected file.
6. **Review gate:** I show you a sample (first 20 rows) and the reject list; you approve before any integration code is written.
7. **Storage:** `catalog/torrents.csv` in a user-selected data folder, gitignored, never inside the installer.
8. **App behaviour:** a local-only matcher reads the CSV, scores rows by normalized title + year + season/episode, and presents a shortlist. The existing `source_set` command sends the chosen magnet to qBittorrent. No network lookup at runtime.
9. **Tests:** CSV parser with hostile rows (formula injection, `javascript:`, oversized fields, duplicate hashes), matcher tests for exact/near/missing matches, and a path-free DTO assertion.

**Status:** not built. Do not build it and the Miauflix-style provider at the same time — pick one.

---

## 6. The Miauflix repo you linked — assessment

I cloned `https://github.com/maury91/miauflix.git` into a temp folder and read its README, source-service docs, planner, sufficiency, candidate normalizer and provider registry.

**What it actually does**
- It requires a **TMDB read access token** for its catalog — the thing you cannot get. So we cannot use it wholesale.
- For sources it does **not** hard-code a site list in the app. It connects to **Jackett or Prowlarr Torznab endpoints you supply**, with an API key where needed. It also supports Cardigann definition files fetched from configured repositories, plus anime-specific providers (AnimeTosho, Nyaa).
- Selection: normalized release names, hash dedupe, IMDb id / season / episode checks, seeder/health thresholds, quality preference, provider priority, request budgets, cooldowns, caching, single-flight, and fallback to another source when one is unusable. Results stream as providers respond.
- README explicitly says "open a movie or episode **you have permission to watch**" and recommends a VPN for default source searches.
- License is **PolyForm Noncommercial** — we may learn from the design, not copy its code.

**What we can adopt lawfully**
- The *pattern*: an operator-supplied Torznab endpoint (their own Jackett/Prowlarr), a client that searches by IMDb id/title/season/episode, candidate normalization and dedupe by info-hash, seeder/health and quality ranking, provider priority, request budget/cooldown, and fallback to the next candidate.
- Our existing qBittorrent container remains the downloader; the executable guard, ffprobe and Defender gates stay in front of playback.

**What we cannot adopt**
- Bundling the same torrent websites / Cardigann definitions / built-in indexers into our app. `AGENTS.md` says **"No content-source bundling. No trackers, indexers, catalogues or instructions for unlawful acquisition. Ever."** Copying Miauflix's provider list would break that non-negotiable, even though Miauflix itself does it. The lawful equivalent is your own Jackett/Prowlarr instance with sources you are entitled to use; the app only receives a Torznab URL and key.
- Any code copy (PolyForm Noncommercial).

**Recommended next architecture (if you choose this)**
- New Rust crate `crates/source-search` (or a module in `crates/acquisition`) implementing an `SourceProvider` port.
- First implementation: `TorznabProvider` — operator configures `baseUrl` + optional key in Settings; backend validates with a bounds-checked request; no site names or URLs ship in the repo.
- `Candidate { hash, magnet, title, year, season, episode, seeders, size, quality }`; normalization reuses a Rust equivalent of Miauflix's NFKD/lowercase/strip approach.
- Ranking: exact IMDb-id match > title+year > title only; require minimum seeders; prefer 1080p/4K for movies; require exact season/episode for TV; reject non-video categories.
- Budget: bounded requests per minute, TTL cache, cooldown on failure, max 2 providers attempted per request initially.
- UI: a shortlist in Details with "Send to downloader" using the existing `source_set`/acquisition command; never auto-play an unverified source.
- ADR required before adding any of this (new provider port, new config surface, new egress).

---

## 7. Where to proceed

**Decision you need to make:** local CSV catalog (§5) or Miauflix-style Torznab provider (§6). Both are operator-supplied-source designs and compatible with our qB downloader; only one should be built first.

If **Torznab**: I need your Jackett/Prowlarr Torznab URL and, if required, an API key. Then: write the ADR → plan with `writing-plans` → implement with subagent-driven development → test with fakes (no live indexer call until you provide one) → add a Settings field → shortlist UI.

If **CSV**: give me the site URL and I will scrape one page with Firecrawl, show you the sample table, and only then build the importer/matcher.

Either way, before calling anything complete:
- `cargo fmt --all -- --check`, clippy `-D warnings`, full workspace tests.
- Frontend six gates.
- Honest `NOT RUN` labels for live indexer/download tests.
- Update `roadmap.md` and `progress.md`, then decide whether to merge `tmdb-artwork-and-exe-guard` into `master`.

**Open gates you own:** human checkpoint sign-off, second-machine LAN proof, TV-distance/dropped-frame review, clean-machine installer test, and a lawful source for a live download test.
