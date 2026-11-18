# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Product app: React 19 + TypeScript + Vite 7 + React Router 7 in `apps/desktop`,
against a frozen typed command contract with a development-only mock transport
(ADR-0009). This surface (`docs/design/tv-ui`) is a static HTML/CSS/JS prototype
with no build step and no dependencies — the user asked for "the static TV
designs, the TV interfaces" from their Figma export, reviewable in a browser
before product integration. The prototype does not change the product stack.

## Users

A non-technical Windows user in a living room, on a couch, roughly three metres
from a television driven by a spare Windows laptop over HDMI. They browse and
play media they own with a remote, controller, keyboard or mouse; they never
see Docker, WSL or command-line concepts. Secondary audience: the product owner
reviewing design implementations in a desktop browser.

## Product Purpose

MEDIA_HAVEN turns a spare or older Windows laptop into a local-first personal
media appliance: discover/import owned media, browse it through a polished
full-screen TV interface, and play it on the host with resume positions, queue,
Up Next and watchlist. Success = a couch-usable, dependable appliance that is
free, open source and understandable without technical help.

## Positioning

A local appliance, not a streaming service: no cloud account, no subscription,
no telemetry, no AI at runtime. Everything that decides, stores or authorises
runs in a mandatory Docker boundary; playback pixels run on the Windows host
(ADR-0002). The interface quality bar is a modern TV streaming app, applied to
media the user already owns.

## Operating Context

Loopback-only services on one Windows laptop; full-screen TV mode plus laptop
and narrow-window breakpoints. Inputs: keyboard arrows + Enter/Space/Escape,
mouse, game controller, common TV remote key events. Mouse supported, never
required; hover never reveals essential controls. Dark room, bright artwork.

## Capabilities and Constraints

- MVP: TV home + library browsing, local import, metadata/artwork with offline
  fallback, playback + resume + completion, queue/Up Next, watchlist, search,
  downloads/storage visibility, settings, setup/diagnostics, safe cleanup.
- TV navigation contract (`docs/design/navigation-spec.md`): exactly one
  focused element; deterministic spatial focus; shelf edge clamping; column
  preservation between shelves; overlay stack with Escape unwinding one level
  and focus restoration to the originating element.
- TV typography: no essential text below 16 CSS px in TV mode; tabular numerals
  for times/counts; full titles on details.
- Motion: focus ≈140 ms, overlays ≈180 ms, hero ≈240 ms; two easing curves;
  `prefers-reduced-motion` zeroes non-essential motion; nothing autoplays on
  focus (no trailers, no audio).
- State is never colour-only: colour plus text, icon or shape.
- No bundled content sources, trackers or catalogues of unlawful acquisition.
- Open decisions (repo O-01): final project branding/package ids; the Figma
  frames carry the name MEDIA_HAVEN and a placeholder logo from the community
  file, treated here as a wordmark slot, not a confirmed brand asset.

## Brand Commitments

- Name in use: MEDIA_HAVEN (Figma frame names, repo docs).
- Binding product identity: MEDIA_HAVEN. Current prototype visual direction is
  guided by `ui ref/ref1.jpg` and `ui ref/ref2.jpg`: near-black ground, centered
  floating glass navigation, full-bleed cinematic hero with left-aligned title
  and actions, landscape Up Next cards, and the following shelf visible below.
  These references guide presentation only; they are not assets or authority to
  change product behavior or branding.
- Artwork: use only existing local prototype artwork and synthetic seeded
  catalogue data. Reference screenshots are comparison inputs, never production
  artwork.

## Evidence on Hand

- Visual references: `ui ref/ref1.jpg`, `ui ref/ref2.jpg` (comparison only;
  not shipped assets).
- Prototype artwork: `docs/design/tv-ui/assets/` (existing local assets with
  provenance recorded per file).
- Product/engineering truth: `Development.md`, `roadmap.md`, `progress.md`,
  `docs/design/design-system.md`, `docs/design/navigation-spec.md`.
- Existing React frontend with mock fixtures: `apps/desktop/src/services/mock/`.
- Absent: real library data, real brand logo asset, backend, measured contrast
  and performance numbers. None may be fabricated as fact.

## Product Principles

1. Artwork is the interface; surfaces stay quiet so posters carry the colour.
2. Legible at three metres — sizes set for TV distance, not a desk.
3. Motion is a signal, not decoration; it shows where focus moved.
4. Local-first and safe by default; infrastructure stays invisible.
5. One system, one scale — no per-screen spacing or colour inventions.

## Accessibility & Inclusion

WCAG AA contrast for normal text against its actual rendered surface (hero
text verified over both bright and dark artwork, not the gradient average).
Focus always visible, including after mouse input, and never colour-only.
Complete keyboard-only path through every screen and overlay.
`prefers-reduced-motion` respected. Essential TV text ≥16 CSS px.