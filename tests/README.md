# tests

Cross-cutting test suites and fixtures. Unit tests live beside the code they
test; this directory holds the tests that need more than one layer.

**Status: partially populated.** Cross-cutting suites currently present:
`tests/dependency_graph.sh` (H3 assertion script), `tests/test-media/README.md`
(gitignored fixtures placeholder), plus crate-level suites that carry the
boundary work: `crates/core/tests/auth.rs`, `crates/contract/tests/{parity,route_parity}.rs`,
`crates/media-library/tests/{roots,scanner}.rs`, `crates/metadata/tests/nfo.rs`,
`crates/persistence/tests/{migrations,repository}.rs`, `crates/app-core/tests/domain.rs`
and `apps/desktop/src/**/*.test.tsx`. `tests/security/`, `tests/performance/` and
`tests/e2e/` are not yet created.

## Layout

| Path | Contents | Created by |
|---|---|---|
| `integration/` | SQLite, config, scanner, migration tests | P1-04, P1-05, P2-02 |
| `security/` | Path escape, boundary enforcement, redaction absence tests | P5-04, P5-06 |
| `performance/` | Idle RAM/CPU, startup, 10k-library timing | P0-04, P2-05, P5-01 |
| `e2e/` | Real app: import → browse → play → resume → uninstall | P3-02 onward |
| `fixtures/` | Generated and hand-made test data | P2-02 |
| `test-media/` | Local-only media for the codec matrix | P3-05 |

## Non-negotiables

- **No personal data.** Fixtures contain generated names and synthetic content.
- **No copyrighted media in git.** `tests/test-media/` is gitignored except for
  its README; contributors place their own files there for a codec run.
- **No secrets, tokens or real endpoints** in any fixture.
- **Absence assertions, not presence assertions.** A redaction test asserts the
  sensitive value is *not* in the output. A boundary test asserts *no* access
  occurred outside the root — not merely that an error was returned.

## Running

```bash
cargo test --workspace                    # observed PASS 2026-11-19 (107 passed, 1 ignored)
bash tests/dependency_graph.sh            # read-only assertion script, H3
cd apps/desktop && npx vitest run         # observed PASS 2026-11-19 (11/11)
cd apps/desktop && npm run check:bundle   # dev-token/signing markers absent from dist/
npx playwright test                       # e2e harness — NOT RUN, suite not yet built
```

Results must be recorded in `progress.md` as PASS / FAIL / **NOT RUN**.
`NOT RUN` is never reported as passing. See `docs/testing/test-matrix.md`.
