# Series and Anime Download-to-Play Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a TV show or anime series picked from the catalog use the same search → download → progress → Play Now flow that movies already use.

**Architecture:** The backend already treats every media id the same way; only the Details page gates the flow behind `movieNeedsDownload`. Extend that condition to a series with no indexed episodes, keep the existing episode Play link when episodes do exist, and bind the source to the series id (which the live backend already accepted). No new route, command, service, dependency or schema change.

**Tech Stack:** React 19 + TypeScript, Vitest + Testing Library, React Router, existing named Tauri commands.

**Spec:** None written. The binding request is the human message "when the user clicks on a show ... is the backend actually able to search download and then play the file with the movie, tv show or anime", followed by "use the writing-plans skill ... then start the execution of the plan using subagent development and parallel execution". The accepted decision is the auto-find-and-confirm flow chosen earlier: pressing Play is the human confirmation of the top shortlist source.

## Global Constraints

- Do not change audit findings 4 or 5 (host credential boundary, container supervisor wiring). The human explicitly said not to change them.
- No new dependency, runtime, route, command, schema or ADR. Reuse `api.sourceSearch`, `api.sourceSet`, `api.downloadStatus` and `api.playbackStart`.
- Use tokens only in CSS. No literal colour or spacing in component CSS.
- Follow TDD vertical slices: one failing test, one minimal implementation, one green run per slice.
- Preserve the existing movie flow and the existing series-with-episodes flow exactly.
- Every Play Now link must use the series id (`/player/${media.id}`), not an episode id, when no episodes exist.

---

### Task 1: Series and anime acquire flow in Details

> Correction (2026-11-26): the movie search → download → Play Now UI this task extends was carried
> uncommitted into the worktree. It entered history as part of `dfa403d` and this fix round, not in
> a 2026-11-25 commit. The 2026-11-25 `progress.md` entry described work that was not yet committed.

**Files:**
- Modify: `apps/desktop/src/pages/Details/MediaDetailsPage.tsx:368-370,414,424`
- Test: `apps/desktop/src/pages/Details/MediaDetailsPage.test.tsx` (append to `describe("Details acquisition progress")`)

**Interfaces:**
- Consumes: `detail` (`SeriesDetail` has `kind`, `title`, `year`, `seasons`; no `availability`), `episodes`, `media.id`, `startPlayback()` from the existing movie flow, `AcquisitionStage`.
- Produces: `needsDownload` boolean used by the Play control; series sources bound to the series media id; Play Now href `/player/:seriesId`.

- [ ] **Step 1: Write the failing test**

Append this test to `describe("Details acquisition progress")` in `apps/desktop/src/pages/Details/MediaDetailsPage.test.tsx`. It uses the existing `SERIES_DETAIL` fixture and declares its own `flush()` helper:

```tsx
  it("finds a source for a series with no episodes and offers Play Now", async () => {
    detailResult = SERIES_DETAIL;
    episodeResult = [];
    await mount("s1", "A Local Series");

    let polls = 0;
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "library_episodes":
          return episodeResult;
        case "source_search":
          return searchResult;
        case "source_set":
          return { downloadId: "d-series" };
        case "download_status":
          polls += 1;
          return [
            {
              id: "d-series",
              torrentHash: "h-series",
              mediaId: "s1",
              state: polls === 1 ? "downloading" : "ready",
              progress: polls === 1 ? 0.25 : 1,
            },
          ];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    const flush = () =>
      act(async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
      });
      await flush();
      expect(invoke).toHaveBeenCalledWith("source_search", {
        title: "A Local Series",
        year: 2023,
      });
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "s1",
        source: MAGNET,
      });

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("status").textContent).toContain("25%");

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("link", { name: /play now/i })).toHaveAttribute(
        "href",
        "/player/s1",
      );
    } finally {
      vi.useRealTimers();
    }
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/desktop && npx vitest run src/pages/Details/MediaDetailsPage.test.tsx`
Expected: FAIL — `screen.getByRole("button", { name: /^play$/i })` cannot find a button; the series currently renders a disabled span "No episodes indexed".

- [ ] **Step 3: Write the minimal implementation**

In `MediaDetailsPage.tsx`, replace:

```tsx
  const availability = detail?.kind === "movie" ? detail.availability : null;
  const movieNeedsDownload =
    !isSeries && availability !== null && availability.state !== "available";
```

with:

```tsx
  const availability = detail?.kind === "movie" ? detail.availability : null;
  const seriesWithoutEpisodes = isSeries && !playEpisode;
  const needsDownload =
    (availability !== null && availability.state !== "available") ||
    seriesWithoutEpisodes;
```

Then replace both occurrences of `movieNeedsDownload` in the Play control (conditions at the first and second branches of the ternary) with `needsDownload`. Do not change `startPlayback`, the shortlist, the manual source field, or the series-with-episodes Play link.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/desktop && npx vitest run src/pages/Details/MediaDetailsPage.test.tsx`
Expected: PASS — 10 tests, including the new series acquisition test.

- [ ] **Step 5: Run the whole frontend gate set**

Run from `apps/desktop`: `npm run typecheck`, `npm run lint`, `npx vitest run`, `npm run check:tokens`, `npm run build`, `npm run check:bundle`.
Expected: all exit 0; Vitest 94/94 across 18 files; tokens 86/0.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/pages/Details/MediaDetailsPage.tsx apps/desktop/src/pages/Details/MediaDetailsPage.test.tsx
git commit -m "feat(details): acquire and play catalog series without local episodes"
```

---

### Task 2: Verify the changed task and the live chain

**Files:**
- Modify: `progress.md` (append one verification entry only)

**Interfaces:**
- Consumes: the commit from Task 1.
- Produces: recorded evidence that the series flow is test-green and the backend still accepts a series id for `playback_start`.

- [ ] **Step 1: Confirm no regression in the Rust/contract surface**

Run from repo root with a fresh target dir: `CARGO_TARGET_DIR=target/qa-series cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo test -p mediahaven-contract --test route_parity`.
Expected: all exit 0, parity 4/4. No Rust file should have changed in Task 1; if one did, stop and report it.

- [ ] **Step 2: Record evidence and commit the note**

The controller, not this implementer, owns the live signed check against `127.0.0.1:8080`; it was already observed HTTP 200 for media id `catalog-series-de6e8643ef7d341629b11966ee109825` earlier on 2026-11-26. Do not start Docker or change containers here.

Append a short dated entry to `progress.md` with the commands, observed results and NOT RUN items. Then:

```bash
git add progress.md
git commit -m "docs: record series acquisition verification"
```
