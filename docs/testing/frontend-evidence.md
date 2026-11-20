# Frontend acceptance evidence — CP-4 (2026-11-20)

Scope: **evidence and documentation only**. This file records what was actually
observed for the CP-4 frontend acceptance criteria. It does **not** pass CP-4.

- Repository: `C:\Users\Stanley\Downloads\MEDIA-HAVEN`
- HEAD: `7d48507`
- Date: 2026-11-20
- Task prefix: `docs/testing/frontend-evidence.md` (this file) and
  `.superpowers/sdd/2026-11-20-evidence/frontend-report.md`
- Live-UI smoke is owned by a separate agent and is **not** duplicated here.

Result legend: **PASS** (observed), **FAIL** (observed, does not meet criterion),
**NOT RUN** (not executed — reason stated). No result here is inferred from a
document or a prior session.

---

## Summary

| # | Item | Task | Result | One-line finding |
|---|------|------|--------|------------------|
| 1 | Computed token contrast ratios | P4-01 | **FAIL** (partial) | 19/25 pairs meet AA-normal; 6 do not — but 5 of those 6 are not actually used as text; one real text token (`--paper-80`) is undefined. |
| 2a | Backend 10k search budget test | P4-08 | **PASS** | `searching_10k_items_is_within_budget ... ok`, test 1.68 s; fresh-target wall 55.21 s. Backend only. |
| 2b | Frontend 10k render | P4-08 | **NOT RUN** | No `library-10k` fixture exists in the repo; needs generated fixture + live app. |
| 3a | Vitest suite (all) | P4-03 | **PASS** | 13/13 across 4 files; **no focus-navigation test suite exists at HEAD**. |
| 3b | Full keyboard-only walkthrough of every screen | P4-03 | **NOT RUN** | Needs a human/Playwright across all screens; the Playwright agent covers a smoke only. |
| 4 | Dropped-frame counts + 3 m TV-distance review | P4-05 | **NOT RUN** | Needs real TV hardware at 3 m; no numbers fabricated. |
| 5 | Containerised-backend end-to-end pass | P4-08 | **NOT RUN** | Needs running workspace + seeded data + UI; prior verification was host-run backend only. |

No criterion in this report is a CP-4 pass. CP-4 remains **OPEN**.

---

## 1. Contrast ratios (P4-01)

### Method

Computed with WCAG 2.1 relative luminance, no dependency added:

```
c_lin = c/12.92                  if c <= 0.03928
c_lin = ((c+0.055)/1.055)^2.4    otherwise
L     = 0.2126*R_lin + 0.7152*G_lin + 0.0722*B_lin
ratio = (L_max + 0.05) / (L_min + 0.05)
```

Translucent text (`--paper-90/-72/-54/-38/-22`) was alpha-composited over the
surface before measuring:
`out = a*fg + (1-a)*bg` per channel. Thresholds: **4.5:1** normal text,
**3:1** large text.

The script was a throwaway in the approved temp dir
(`C:\Users\Stanley\AppData\Local\Temp\opencode\contrast_check.py`), not added to
the repository and not a new dependency. Input values are the literal tokens in
`apps/desktop/src/styles/tokens.css` (verified against that file).

Command:

```
python "C:\Users\Stanley\AppData\Local\Temp\opencode\contrast_check.py"
```

### Observed ratios (verbatim output)

```
PAIR                                                   RATIO   AA-normal  AA-large
-----------------------------------------------------------------------------------
--paper on --ink-void                                  19.61        PASS      PASS
--paper on --ink-950                                   19.04        PASS      PASS
--paper on --ink-900                                   18.71        PASS      PASS
--paper on --ink-800                                   17.66        PASS      PASS
--paper on --ink-700                                   16.76        PASS      PASS
--paper on --ink-500                                   13.64        PASS      PASS
--paper-90 (a=0.93) over --ink-void                    16.79        PASS      PASS
--paper-72 (a=0.8) over --ink-void                     12.25        PASS      PASS
--paper-54 (a=0.64) over --ink-void                     7.85        PASS      PASS
--paper-38 (a=0.5) over --ink-void                      5.00        PASS      PASS
--paper-22 (a=0.26) over --ink-void                     2.03        FAIL      FAIL
--paper-90 (a=0.93) over --ink-900                     16.10        PASS      PASS
--paper-72 (a=0.8) over --ink-900                      11.88        PASS      PASS
--paper-54 (a=0.64) over --ink-900                      7.75        PASS      PASS
--paper-38 (a=0.5) over --ink-900                       5.04        PASS      PASS
--paper-22 (a=0.26) over --ink-900                      2.13        FAIL      FAIL
--accent-ink on --accent (white button)                19.92        PASS      PASS
--nav-active-ink on --nav-active-bg                    17.27        PASS      PASS
--success on --ink-void                                12.21        PASS      PASS
--danger on --ink-void                                  8.26        PASS      PASS
--warning on --ink-void                                12.67        PASS      PASS
--tone-ink on --tone-1                                  3.89        FAIL      PASS
--tone-ink on --tone-2                                  3.93        FAIL      PASS
--tone-ink on --tone-3                                  4.13        FAIL      PASS
--tone-ink on --tone-4                                  4.34        FAIL      PASS
-----------------------------------------------------------------------------------
Total pairs: 25 | failing AA-normal (<4.5): 6 | failing AA-large (<3.0): 2
```

Additional pair, because avatar text is `--paper` on the tone backgrounds
(`components/nav/nav.module.css:158`, `pages/Profiles/profiles.module.css`):

```
--paper on --tone-1: 4.70:1  AA-normal=True
--paper on --tone-2: 4.65:1  AA-normal=True
--paper on --tone-3: 4.42:1  AA-normal=False
--paper on --tone-4: 4.21:1  AA-normal=False
```

### Failures and their real-world impact

| Pair | Ratio | Impact |
|------|-------|--------|
| `--paper-22` (0.26) over `--ink-void` / `--ink-900` | 2.03 / 2.13:1 | **Not used as text.** Grep shows it only as `background` (`setup.module.css:112`) and a focus ring `box-shadow` (`setup.module.css:183`). Fails the 3:1 non-text UI-component threshold, so the ring is weak, but it is not a text-contrast failure. |
| `--tone-ink` on `--tone-1..4` | 3.89–4.34:1 | `--tone-ink` is **defined but never used** as text. All four pass the 3:1 large-text threshold. |
| `--paper` on `--tone-3` / `--tone-4` | 4.42 / 4.21:1 | **Real, marginal AA-normal failure** for small avatar initials on those two tone backgrounds. Large text (≥18 px / ≥14 px bold) would pass. |

### Defect found during this check

`--paper-80` is referenced as **text colour** in four places but is **not
defined anywhere** in `tokens.css` (only `--paper-90/-72/-54/-38/-22` exist):

- `components/hero/hero.module.css:135`, `:164`
- `components/nav/nav.module.css:126` (`.action` icon colour)
- `components/ui/category.module.css:42`

An undefined custom property with no fallback makes `color:` invalid at
computed-value time, so those elements inherit their colour rather than getting
the intended 0.80-alpha paper. This is a token typo, not a measured contrast
result, and it is recorded here as a concern for the owning task.

**Verdict for P4-01 contrast criterion:** partially evidenced. The primary
body/accent/status text pairs all meet AA. Three small-text cases do not
(avatar tone-3/4, plus the undefined `--paper-80` path). No contrast was checked
over real artwork — the roadmap lists that as a risk and it needs the live UI.

---

## 2a. 10k-item search timing — backend (P4-08)

Command (fresh `CARGO_TARGET_DIR`, forced recompile; no install, no config change):

```
$env:CARGO_TARGET_DIR="target/evidence-10k-20261120"
cargo test -p media-persistence searching_10k_items_is_within_budget -- --ignored
```

Observed output (compile lines abbreviated where marked, otherwise verbatim):

```
   Compiling proc-macro2 v1.0.107
   ... (full dependency rebuild in the fresh target dir) ...
   Compiling media-persistence v0.1.0 (C:\Users\Stanley\Downloads\MEDIA-HAVEN\crates\persistence)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 50.97s
     Running unittests src\lib.rs (...\media_persistence-...exe)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\migrations.rs (...\migrations-...exe)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
     Running tests\repository.rs (...\repository-...exe)
running 1 test
test searching_10k_items_is_within_budget ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 1.68s
WALL_SECONDS=55.21
```

Notes:
- PowerShell emitted `NativeCommandError` noise for cargo's stderr progress
  lines; the test itself reported `ok`, so the run succeeded.
- The **test body** took 1.68 s. The **fresh-target wall time** was 55.21 s,
  dominated by a 50.97 s recompile. These are different measurements; neither
  is a frontend render time.
- This is the SQLite search path in `crates/persistence`, i.e. **backend-side
  only**. It does not satisfy the P4-08 "10 000-item frontend check" or the
  P2-04/P4-08 "render without eager-loading every image" criterion.

## 2b. Frontend 10k render — NOT RUN

Reason: no `tests/fixtures/library-10k` fixture exists in the repository (the
test matrix says it is generated, not committed if large), and no browser/UI
harness was run in this scope. Requires: generate a 10 000-item fixture, serve
it through the transport seam, and measure image requests + interaction timing
in the running app. No number is claimed.

---

## 3a. Vitest suite (P4-03 context)

Command (from `apps/desktop`):

```
npx vitest run
```

Observed output (verbatim, ANSI formatting stripped):

```
 RUN  v3.2.7 C:/Users/Stanley/Downloads/MEDIA-HAVEN/apps/desktop

 ✓ src/services/transport.test.ts (4 tests) 8ms
 ✓ src/pages/Player/PlayerPage.test.tsx (2 tests) 1178ms
   ✓ player driven by the engine > routes seek and pause through the named player_control command 958ms
 ✓ src/App.test.tsx (2 tests) 1545ms
   ✓ app shell > opens on profile selection when no profile is active 993ms
   ✓ app shell > enters the library after a profile is chosen 548ms
 ✓ src/pages/Queue/QueuePage.test.tsx (5 tests) 2445ms
   ✓ queue screen > renders entries in the order returned by the backend 783ms
   ✓ queue screen > adds through the named command without de-duplicating in the UI 693ms
   ✓ queue screen > reorders via queue_reorder and re-renders the new order 377ms
   ✓ queue screen > removes only after confirmation, using the queue id 484ms

 Test Files  4 passed (4)
      Tests  13 passed (13)
   Start at  08:47:04
   Duration  40.22s (transform 2.97s, setup 2.52s, collect 10.99s, tests 5.17s, environment 18.57s, prepare 3.88s)
```

**Important discrepancy.** The task brief describes an "existing Vitest
focus-navigation suite [covering] horizontal/vertical/overlay restoration."
No such suite exists at HEAD `7d48507`. The four test files present are
`transport.test.ts`, `PlayerPage.test.tsx`, `App.test.tsx`, `QueuePage.test.tsx`;
none contains a focus/spatial/overlay-restoration test (grep over
`*.test.*` for `focus|spatial|navigation|overlay` found no structural matches).
`progress.md` records an earlier 8-test focus suite from the pre-wiring redesign
(2026-11-18), but that UI was replaced in the "UI consolidation and backend
wiring" step, and the suite is not in the current tree. The 13/13 PASS above
therefore says **nothing** about P4-03 focus behaviour.

## 3b. Full keyboard-only walkthrough — NOT RUN

Reason: needs a human or Playwright to drive Home → shelf → card → details →
player → back → settings → back with arrow keys/Enter only, across every screen,
and record focus restoration for each overlay. Out of scope here; the separate
Playwright agent covers a browser smoke only, which is not a full walkthrough.
No step log or PASS is claimed.

---

## 4. Dropped-frame counts / 3 m TV-distance review (P4-05) — NOT RUN

Reason: requires the real target TV and a viewer at 3 m on the lowest supported
hardware to measure frame drops and whether focus is identifiable. No device
profile is present in this evidence run, so no numbers are fabricated.

Static token observation only (not a hardware measurement): `tokens.css`
defines `--text-micro: clamp(11px, 0.8vw, 14px)` and
`--text-small: clamp(14px, 0.95vw, 17px)`. The P4-05 criterion "no essential
text below 16 CSS px in TV mode" is therefore at risk if these tiers carry
essential text; whether they do needs the live UI review, so the criterion is
left NOT RUN rather than failed here.

---

## 5. Containerised-backend end-to-end (P4-08) — NOT RUN

Reason: the frontend was previously verified against a **host-run** backend
(`cargo run -p mediahaven-core`), not the mandatory `core`/`probe` containers.
An end-to-end pass requires all of: a running Docker/WSL2 workspace with
`core` healthy and no mock in the bundle, seeded library data, the UI started
against that origin, and the failure-mode suite (backend down, token rejected,
root offline). None of that was executed here — no container or host
configuration was touched, per scope. No PASS is claimed.

---

## Concerns / open items

1. `--paper-80` undefined but used as text in 4 CSS modules (see §1).
2. Small-text `#tone-3`/`#tone-4` avatar initials miss AA-normal by 0.08/0.29.
3. The focus-navigation test suite assumed by the task brief is absent at HEAD;
   P4-03 has **no automated focus coverage** in the current tree.
4. P4-05 minimum text size and contrast-over-artwork need real-device review.
5. Fresh-target compile (50.97 s) is not a search-time metric; quote the 1.68 s
   test body when discussing the budget, and never as a frontend render time.

## Sign-off

**CP-4 is NOT passed by this report.** Evidence was gathered for 7 items; only
3 produced an observed PASS/FAIL, and 4 are NOT RUN with reasons above. A
checkpoint pass additionally needs the separate live-UI smoke, the full keyboard
walkthrough, real-hardware frame numbers, and human review — none of which is
claimed here.
