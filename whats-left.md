# What is left before we can release

Plain-language list. Date: 2026-11-22.

## Where we are now

The app can: read a local library, show real posters, browse a movie / TV / anime
catalogue from TMDB, start its own backend, save your TMDB key and Prowlarr
details, add working torrent sites to Prowlarr with one click, and show a list
of sources on a movie page.

We proved parts of it with real data: posters loaded, the source setup added
four sites, and searching found 10 real results for Arrival and 10 for Dune
with working magnet links.

**This is not ready for other people to test yet.** The items below are still
open.

---

## A. Things that stop us from giving the app to users

1. **We have never started the installed app on a real cold machine.**
   We only started the backend by hand with commands. The part that starts it
   when you open the app has not been tried inside a real install. If it fails,
   the app will just show "service down".

2. **The app was driven in a browser, but not viewed on a TV or desktop window.**
   Playwright clicked through Home, Movies, Details, Discover, Anime, Setup
   and the Player page; posters rendered. Four bugs were found and fixed.
   Details: `docs/testing/browser-qa-2026-11-23.md`. No axe run or visual
   baseline yet, and no native mpv window in the browser.

3. **A chosen torrent has downloaded, but not from the internet.**
   The full app path was tested with two locally made torrents and a local
   tracker. qBittorrent really downloaded an 86 KB file. An external torrent
   and the following ffprobe/Defender gates are still untested.

4. **The `.exe` guard was tested with real qBittorrent.**
   A torrent containing `installer.exe` was rejected; no executable was
   written. Live testing found and fixed a wrong qB endpoint (`/resume` →
   `/start`) that would otherwise have rejected safe torrents.

5. **One saved site does not work.**
   EZTV is blocked by a website guard. The other four are active: YTS, Nyaa,
   The Pirate Bay and Anime Tosho. Anime Tosho is a real anime source added
   through Prowlarr's own definition. The earlier "no working address" note
   was wrong — the address is `feed.animetosho.net`, not `.org`.
   Two other sites from the Miauflix list, RARBG and Torrentio, are not in
   Prowlarr. They would need a separate search engine in our app, so they are
   not included.

6. **The installer has not been tried on a clean Windows computer.**
   Previous installer checks used this machine. A fresh machine may be missing
   something. The installer is also unsigned, so Windows may warn users.

7. **No checkpoint has been signed off.**
   The project has gates CP-0 to CP-7. None has been passed by a human.
   Without that, we cannot call this a release.

---

## B. Things we must check before user testing

1. **Second computer.** Confirm the local service cannot be reached from
   another machine on the network. We have only checked on this one.

2. **TV use.** Sit at TV distance, use the remote-style keys, and check that
   text is readable and nothing drops frames. Not done.

3. **Full keyboard use.** Arrow keys, Enter and Escape were tested in code
   tests, but not by a person using the whole app.

4. **Real library.** We tested with 10 sample titles. A real folder with
   hundreds of files has not been scanned. Storage limits and old-poster
   cleanup could behave differently.

5. **Profiles and watchlist on a fresh install.** These work in the sample
   data, but a new user with no data has not been walked through setup.

6. **The "Find sources" list with a slow or broken site.** We tested with
   working sites. If a site is slow, the app waits up to 20 seconds per site.
   That may feel slow. We have not tested the worst case.

---

## C. Known small problems we can live with for now

- The catalogue shows at most 12 posters per page on purpose, so later cards
  may have no picture until you reload.
- Anime comes from a TMDB filter, not a real anime catalogue. Some anime will
  be missing.
- Prowlarr results sometimes have two different years in the title. The app
  may accept one. It never downloads by itself, so a person still chooses.
- If a result has a bad magnet link, the app drops it even if another link is
  present. Better to show fewer choices than a broken one.
- Some names with symbols may appear with raw `&amp;` instead of `&`.
- The dev-only version shows site names in a test helper. The real app shows
  only counts and reasons.
- A few older settings switches still say "Not available" (OMDb age ratings
  and offline IMDb datasets). They do not affect posters or downloads.
- The startup retry lasts about 29 seconds. A very slow computer might still
  need a manual Retry click.
- The Windows path conversion does not handle network folders. Our temp folder
  is a normal drive path, so this should not happen.
- Some roadmap checkboxes are still unticked because their live gates are not
  done. That is honest, not an oversight.

---

## D. Decisions still open

1. **Merge this branch into `master`.** All work is on
   `tmdb-artwork-and-exe-guard`. It is not on the main branch yet.
2. **Sign the installer.** Current plan is to ship unsigned and warn users.
   Signing needs a certificate and a human budget decision.
3. **Add FlareSolverr** if EZTV must work. That means another install.
4. **AnimeTosho is resolved.** It works through Prowlarr's native `Anime Tosho`
   definition; no extra service is needed.
5. **RARBG and Torrentio are not available** in Prowlarr. Adding them would
   need a new search engine in our app — a separate decision, not a quick fix.
6. **OMDb age ratings: decided to skip for v1.** There is no key and the
   setting still says "Not available". Posters and search do not need it.
   Revisit only if a user asks for age ratings.
7. **Checkpoint sign-off: you sign them, I prepare the evidence.** CP-0 to CP-7
   stay OPEN until you read the recorded results and agree. No agent signs a
   checkpoint on your behalf.

---

## E. What we already proved (so nobody repeats it)

- Backend answers its health check and library calls with signed requests.
- Real posters load from TMDB and are served as JPEG files from local cache.
- TMDB catalogue returned 20 movie, 20 TV and 20 anime entries; 12 of each
  had posters; no key or address appeared in the reply.
- Prowlarr setup found the original three sites already present and skipped
  two with fixed reasons; no key or address appeared in the reply. Anime Tosho
  was then verified and added as the fourth working site.
- Source search returned 10 real magnets for Arrival and 10 for Dune, sorted
  by seeders, with no key or address in the reply.
- The full download path worked with real qBittorrent: a safe 86 KB file was
  downloaded, and a torrent containing `installer.exe` was rejected before any
  file was written. Details: `docs/testing/end-to-end-2026-11-23.md`.
- An unsigned per-user installer was built at
  `target/release/bundle/nsis/MEDIA_HAVEN_0.1.0_x64-setup.exe`, SHA-256
  `2115685f962c1efbeffb12e40bbcbfe16bd6d5a476b8c9fded0ac866a87e9107`.
- All Rust and frontend checks pass. Every feature had a separate worker and
  reviewer, and all review findings were fixed and rechecked.

---

## F. Recommended order

1. Merge this branch into `master`.
2. Start the installed app cold on this machine and confirm the backend comes
   up, posters appear, and the Discover tab works — with a human watching the
   screen.
3. Finish the download path with an external, lawful torrent: after the file
   arrives, observe the ffprobe and Defender gates and playback. The app-side
   download and the `.exe` rejection were already proven locally.
4. Confirm the bad-file check again on a clean machine; the local qB test is
   recorded in `docs/testing/end-to-end-2026-11-23.md`.
5. Check the service cannot be reached from a second computer.
6. Sit at TV distance with a remote-style keyboard and review readability.
7. Try the installer on a clean Windows machine.
8. Get human sign-off on CP-0 to CP-7, then invite the first users.
