# Security policy

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Report privately to the maintainer, including:

- Affected version or commit.
- A description of the issue and its impact.
- Reproduction steps or a proof of concept.
- Any suggested mitigation.

You will get an acknowledgement, an assessment, and a coordinated disclosure
timeline. Please allow time for a fix before publishing.

## Scope

In scope: the MEDIA_HAVEN application, installer, scripts, container
configuration and documentation in this repository.

Out of scope: third-party media files, third-party software and services,
issues requiring local administrator or kernel-level access, and anything
covered by the accepted residual risks in `docs/security/threat-model.md`.

## Supported versions

The project is pre-release. No version is currently supported for production use.
This section will list supported minor versions once the first release ships.

## Security properties this project intends to hold

These are the claims a report should be measured against:

1. **No source-media deletion.** The app never deletes user source media.
2. **Bounded filesystem access.** All operations stay inside user-selected roots
   and the managed storage root; traversal and reparse-point escapes are rejected.
3. **Loopback-only.** No listener binds a non-loopback interface in any MVP mode.
4. **No silent host mutation.** No Windows feature, firewall, admin-rights or
   runtime change without explicit, specific consent.
5. **No telemetry.** Nothing is sent anywhere by default.
6. **Redacted diagnostics.** Exported bundles contain no tokens, passwords,
   personal paths or filenames (filenames only on explicit opt-in).
7. **No bundled content sources.** No trackers, indexers or catalogues.
8. **Least privilege.** The app runs as the normal user and requests elevation
   only for a specific, documented system change.

A violation of any of these is a security bug, not a feature request.

## What is not a vulnerability

- The app reading media files you selected.
- The app refusing to open a file with an unsupported codec.
- Watch history being readable by the same Windows user account.
- A hostile file causing playback to fail safely.
- Container services being absent when you declined to install a runtime.

## Hardening guidance for users

- Keep Windows and the app updated.
- Only add folders you control.
- Do not enable optional services you do not need.
- Review any cleanup preview before confirming.
