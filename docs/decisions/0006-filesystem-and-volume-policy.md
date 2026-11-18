# ADR-0006 — Five storage classes, one read-only media rule

- **Status:** Accepted (provisional, 2026-11-18) — explicit volume mounts, read-only media by default, separate volumes for data/cache/artwork/temp, no unrestricted host access
- **Date:** 2026-11-16
- **Related:** ADR-0001, ADR-0002, `docs/architecture/docker-workspace.md` §4, `docs/security/threat-model.md`

---

## Context

The single largest irreversible failure this product can cause is damaging a user's
original media. The second largest is losing their watch history and library
organisation. These are different problems and must not share storage.

## Decision

Five storage classes, never conflated:

| # | Class | Location | Access | Survives |
|---|---|---|---|---|
| 1 | **Original user media** | the user's own folders, wherever they chose | read-only bind mount into containers; no write path in the codebase | everything |
| 2 | **Application data** | named volume `mediahaven_data` → `/var/lib/mediahaven` | `core` read/write | uninstall-with-keep, upgrades, app restarts |
| 3 | **Temporary processing** | tmpfs in each container | owning container only | nothing (by design) |
| 4 | **Imported / untrusted** | named volume `mediahaven_staging` → `/var/lib/mediahaven/staging` | `probe` writes, `core` promotes | until promoted or purged |
| 5 | **Config and secrets** | Windows Credential Manager + runtime environment | host supervisor | until user clears them |

### The read-only media rule

Source media is mounted `:ro`. There is no "write mode" flag in v1. Any future feature
that must write to a source folder (a sidecar `.nfo`, an optimised version) requires:

1. a new ADR,
2. an explicit per-root user grant on a screen that names the folder,
3. a dry-run preview,
4. and a mandatory backup of anything it touches.

Until then the answer to "can MEDIA_HAVEN modify my files?" is a single word: **no.**

### Why a named volume, not a bind mount, for app data

- A bind mount into `C:\Users\…` would put the database inside the user's profile where
  other software, backup tools and sync clients reach it — and would require mounting a
  host directory, which §2 of the workspace document forbids.
- A named volume lives inside the WSL2 VM's virtual disk, is only reachable through
  `core`, and has one clean backup/restore path.
- Cost: the user cannot open the database in Explorer. Accepted; there is no supported
  reason to.

**Correction — the volume is not unreachable from Windows.** Docker documents that "WSL's
file system is accessible from the Windows host `\\wsl$`, meaning Windows processes can
read and modify files within WSL." A Windows process running as the same user can
therefore read and edit the database inside `core-data` via
`\\wsl$\<distro>\var\lib\docker\volumes\...`. The earlier phrasing "only reachable
through `core`" was too strong and is corrected here.

What the named volume *does* still guarantee:

- **No other container can reach it.** Only `core` mounts `core-data`. `probe` cannot
  read or write the database even if it is fully compromised.
- **The original media is protected by read-only bind mounts**, not by volume obscurity.
  A same-user Windows process can already read the user's media; that is not a privilege
  MEDIA_HAVEN grants or withholds.

The volume is therefore a *containment* boundary between containers, not a confidentiality
boundary against the Windows user. Recorded as R-15. Nothing secret is stored there.

### Mount hygiene

- One bind mount per library root, named `/media/root-<n>`. No broad mounts, ever.
- Deny-list enforced in the supervisor before `compose up`:
  `C:\`, `C:\Users`, `%USERPROFILE%`, `C:\Windows`, `C:\Program Files`,
  `C:\ProgramData`, `\\?\`, `\\.\pipe\`, any drive root, and the WSL2 distro filesystem.
- A root that is a reparse point/junction is refused (checked on the Windows side, where
  reparse points are a real attack surface for path policy).
- Adding or removing a root is a supervisor action that re-renders the compose override and
  restarts `core`. It never widens an existing mount.

## Consequences

- Removing a library root deletes **zero** files and only rows (P1/P2 tests assert this).
- Uninstall offers three separate choices: remove containers, remove data volume, remove
  nothing. Media is never in any of those lists.
- The database is technically inspectable from Windows via `\\wsl$`; it is not *supported*
  as a user-facing interface, and diagnostics go through the API or a supervised backup
  export so that the supported path stays stable. Reading it out of band is an accepted,
  documented possibility, not an accident (R-15).
- Backup is a first-class operation, not a file copy (see below).

## Backup and restore

Because app data lives in a volume, backup is: `core` streams a consistent snapshot
(SQLite `VACUUM INTO` + artwork manifest) to a tar stream; the supervisor writes it to a
user-chosen destination. Restore is the inverse, into a fresh volume, with a version check
and a refusal to restore a newer schema into an older app.

Original media is **not** included in these backups. It is the user's; they already have
it, and duplicating a 2 TB library would be absurd.

## Verification

- Removing a root: file count unchanged before/after (assertion, not inspection).
  Covered by P1-07 and P5-04.
- `docker inspect core` shows exactly the declared mounts and no more.
- Root filesystem is `read_only` and a write to an undeclared path fails.
- Backup → destroy volume → restore → library identical, resume positions preserved.
