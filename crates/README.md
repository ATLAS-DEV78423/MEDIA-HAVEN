# crates

The Rust workspace. Business rules live here — never in the frontend, never in a Compose file. The backend application is deployed in the mandatory Docker `core` container; no host-side backend fallback is permitted.

**Status: workspace scaffolded (P1-01).** `app-core`, `media-persistence` and `mediahaven-core`
exist as workspace members. Domain logic lands in later tasks. `Development.md` §6.2
is explicit: do not create empty scaffolding beyond what a task requires.

## Planned crates and their single responsibility

| Crate | Owns | Must never contain |
|---|---|---|
| `app-core` | Domain entities and invariants (`domain/`), use cases (`application/`), port traits (`ports/`), domain events (`events/`); deployed in `core` | I/O, Tauri types, React |
| `persistence` | SQLite queries and migrations | UI logic, policy decisions |
| `media-library` | Folder import, safe scanning, metadata association | Whole-drive scanning |
| `playback` | Player adapter and playback/completion state | Automatic deletion |
| `queue` | Ordering and transitions | Provider-specific network code |
| `storage-policy` | Limits, retention, safe cleanup, exclusions | Unrestricted filesystem access |
| `metadata` | Provider adapters and local cache | Mandatory network calls |
| `downloads` | Optional provider abstraction | Hard-coded content sources |
| `runtime-adapter` | Windows host supervisor adapter for the required Docker/Compose workspace | Product business rules; frontend runtime access |
| `platform-windows` | Windows APIs, process management, startup | Cross-platform domain logic |
| `diagnostics` | Health snapshot, bundle export, redaction | Network uploads |

## Dependency rule

```
domain  ←  application  ←  api / commands
   ↑            ↑
 ports     infrastructure adapters
```

Arrows point inward only. `domain` depends on nothing outside the standard
library. If a container SDK, a player SDK or `tauri` appears in `domain`, the
architecture has broken — fix it before merging.

Enforced by a CI dependency check (P1-01/P6-02).

## Rules

- One crate, one responsibility. A crate that needs a second reason to exist
  should be split.
- Ports are traits in `app-core/ports`; implementations live in the adapter
  crate. Tests use fakes, never the real adapter.
- `thiserror` for domain errors, `anyhow` at application boundaries only.
- No `unwrap()`/`expect()` on user-controlled input.
- Every path crossing a boundary is canonicalised and containment-checked.

See `docs/architecture/overview.md` for the full layering table.
