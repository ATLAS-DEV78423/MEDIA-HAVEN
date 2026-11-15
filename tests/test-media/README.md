# test-media

**This directory is intentionally empty and gitignored.**

It is where a contributor places their own media files to run the playback codec
matrix (`docs/testing/test-matrix.md`, task P3-05).

## Rules

- Do **not** commit copyrighted media. `tests/test-media/*` is ignored by git;
  only this README is tracked.
- Do not use files from anyone's personal library for a recorded test run.
  Generated or freely licensed samples only.
- Record what you used by *description*, not by file path: "1080p H.264/AAC MP4,
  30 s, freely licensed sample". Never paste a personal path into a report.
- Delete local copies when you are done.

## Suggested set

Generate or obtain small, freely licensed samples covering:

| Container | Video | Audio | Subtitles |
|---|---|---|---|
| MP4 | H.264 | AAC | external SRT |
| MP4 | H.264 | AAC | embedded SRT |
| MKV | H.264 | AC3 | ASS with positioning |
| MKV | HEVC | EAC3 | ASS |
| WebM | VP9 | Opus | none |
| MKV | AV1 | FLAC | none |

Plus, for the failure paths:

- A file with an unsupported codec (to verify the message names the codec).
- A truncated file (to verify it fails safely, not silently).
- A file with a hostile filename (control characters, extreme length).
- A very long file (to test a seek far into the timeline).

## Why this matters

The playback matrix is the evidence behind ADR-0002. If the matrix is populated
by guessing rather than by playing real files, the ADR is not evidence-based and
the release makes a claim it cannot support.
