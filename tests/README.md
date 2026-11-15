# tests

Cross-cutting test suites and fixtures. Unit tests live beside the code they
test; this directory holds the tests that need more than one layer.

**Status: empty.** Suites are created by the roadmap tasks that need them.

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

## Running (once the harness exists)

```bash
cargo test --workspace            # unit + integration
npm test -w apps/desktop          # component tests
cargo test --test security        # boundary suite
npx playwright test               # e2e
```

Results must be recorded in `progress.md` as PASS / FAIL / **NOT RUN**.
`NOT RUN` is never reported as passing. See `docs/testing/test-matrix.md`.
