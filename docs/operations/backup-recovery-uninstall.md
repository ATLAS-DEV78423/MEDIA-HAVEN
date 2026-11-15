# Backup, recovery and uninstall

Derived from `Development.md` §6.1, §9.3, §10.3 and §15.2.

## What lives where

The application database lives in a **named Docker volume** inside the WSL2 distro,
not in `%LOCALAPPDATA%`. See ADR-0006.

| Data | Path | Contains personal data | Backed up |
|---|---|---|---|
| User source media | User-chosen folders | Yes | User's responsibility — the app never touches it |
| Database | Named volume `core-data` (inside the WSL2 distro) | Watch history, resume, queue | **Yes** |
| Staged / untrusted files | Named volume `staging` | Possibly | No — transient by design |
| Host config | `%LOCALAPPDATA%/<AppName>/app/config/` | Folders, limits, preferences | **Yes** |
| Logs | `%LOCALAPPDATA%/<AppName>/app/logs/` | Redacted | No |
| Artwork cache | Named volume `core-data`, `cache/` subtree | No | No — rebuildable |
| Managed media | User-chosen managed root (`incoming/`, `ready/`, `quarantine/`, `trash/`) | Yes, if downloads are used | Optional, user's choice |
| Credentials (if ever) | Windows Credential Manager | Yes | No — re-authenticate |
| Diagnostics | `%LOCALAPPDATA%/<AppName>/app/diagnostics/` | Redacted exports | No |

`<AppName>` is a neutral placeholder until branding is decided (open decision
O-01).

**Rule:** the install directory never holds user data. The container writable
layer never holds the only copy of anything.

**Note:** a named volume is *not* a secret store. WSL exposes distro filesystems to the
Windows host via `\\wsl$`, so a same-user Windows process can read and write the volume
out of band (R-15). The volume is a boundary between containers, not a confidentiality
boundary against the Windows user. Nothing secret is stored there.

## Backup

### Automatic
- The database is copied **before** any migration that could alter data
  (P1-05). The backup is retained until the migration is verified.
- Config is written atomically; the previous valid file is kept as a fallback.

### Manual
A user-facing backup should produce one archive containing the database, config
and (if chosen) managed media. Implemented in Phase 7 alongside the release
gates; documented here so the data layout does not drift.

### Restore
1. Close the app.
2. Restore the database and config to their documented locations: the volume via the
   supervisor's backup/restore path (a `core` snapshot stream, not a file copy of a
   live SQLite file), and the config file to `%LOCALAPPDATA%/<AppName>/app/config/`.
3. Start the app. Pending migrations run against the restored database after
   taking a fresh backup.
4. Verify: library count, recent watch history, queue contents.

A restore must never require touching user source media.

## Recovery after failure

| Failure | Automatic behaviour | User action |
|---|---|---|
| Corrupt config | Quarantine the file, load defaults, warn | Reconfigure; the quarantined file is kept for inspection |
| Failed migration | Roll back, keep backup, enter recovery state | Report with the backup path |
| Corrupt database | Detect on open, offer the most recent backup | Restore or start fresh (explicit) |
| Interrupted scan | Resume on next start, report incomplete state | Rescan a folder if items are missing |
| Interrupted download | Query provider for resume/verify | Retry or cancel |
| Power loss mid-write | Recover to last consistent state on start | None, if recovery succeeded |
| Optional service unhealthy | Bounded retries, then degrade to local mode | Restart the service or disable the feature |

Safe mode starts the UI without optional services so the library remains
reachable when something else is broken.

A recovery operation **never deletes data**. If recovery concludes that data must
be discarded, it requires explicit confirmation with the affected paths shown.

## Uninstall

Implemented by P7-02. The installer must offer a clear choice.

### Default: keep app data
Removes program files, shortcuts, services and scheduled tasks. Leaves
`%LOCALAPPDATA%/<AppName>/` and managed media intact so a reinstall resumes where
the user left off.

### Optional: remove app data
Additionally removes the database, config, cache, logs and diagnostics. Requires
explicit confirmation naming each path.

### Never
- Deletes user source media.
- Deletes a folder the user selected as a library root.
- Leaves orphaned containers, volumes, services, scheduled tasks or firewall
  rules.
- Unregisters a WSL2 distro or removes a container runtime that other
  applications may depend on. If MEDIA_HAVEN owns a dedicated distro, removing
  it is a separate, explicitly confirmed action with a verified backup first.

### Verification
- File count of source media roots before and after: unchanged.
- No leftover service, scheduled task or firewall rule.
- No orphaned containers or volumes.
- App data behaviour matches whichever option the user chose.

## Data-loss guardrails

- Cleanup requires a preview, explicit confirmation, staging to managed trash,
  an audit log entry, and a retention period.
- Automatic permanent deletion is **off by default** and requires multi-signal
  completion evidence (`Development.md` §20.3).
- The completion logic that could eventually authorise deletion is guarded by an
  adversarial test suite (P3-04). A failing test there is a release blocker.
