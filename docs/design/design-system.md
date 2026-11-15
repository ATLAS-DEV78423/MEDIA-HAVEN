# Design system

The visual contract. Derived from `Development.md` §7.3 and §7.4. Every value
here becomes a CSS custom property in `apps/desktop/src/styles/tokens.css`
(P4-01). Components consume tokens; components never contain literals.

## Principles

1. **Artwork is the interface.** Surfaces stay quiet; posters carry the colour.
2. **One system, one scale.** No per-screen spacing inventions.
3. **Legible at three metres.** Sizes are set for TV distance, not a desk.
4. **Motion is a signal, not decoration.** It shows where you are and where
   focus moved. It never runs unattended.
5. **Never colour-only.** State is always colour *plus* text, icon or shape.

## Colour

| Token | Role | Notes |
|---|---|---|
| `--bg-base` | App background | Near-black/charcoal, not pure `#000` |
| `--bg-elevated` | Cards, sheets | Slightly lifted from base |
| `--surface-translucent` | Navigation rail, player overlay | Restrained blur; not every surface |
| `--text-primary` | Titles, primary labels | Must clear AA on every surface it lands on |
| `--text-secondary` | Metadata, timestamps | AA against its own surface |
| `--text-tertiary` | De-emphasised hints | Never the only carrier of essential info |
| `--accent` | Primary action, progress fill | One accent; do not introduce a second |
| `--focus-ring` | Focus indicator | Must survive bright artwork |
| `--danger` | Destructive action | Always paired with a label and confirmation |
| `--success`, `--warning`, `--info` | Status pills | Always paired with text |

**Contrast:** normal text meets WCAG AA (4.5:1) against its actual rendered
surface. Hero text sits on a gradient over unknown artwork — verify against both
a bright and a dark fixture, not against the gradient's average.

**No gradient soup.** Gradients exist to protect text over artwork, not as
decoration.

## Typography

| Token | Use | Guidance |
|---|---|---|
| `--font-display` | Hero title | Large, tight tracking, two lines max |
| `--font-ui` | Everything else | System-adjacent, high legibility |
| `--text-hero` | Featured title | Scales with viewport; never below TV minimum |
| `--text-title` | Section headers | Clear hierarchy step |
| `--text-body` | Descriptions | Line length capped |
| `--text-meta` | Year, runtime, episode count | **Never below 16 CSS px in TV mode** |

Rules:

- Minimum 16 CSS px for any essential text in TV mode.
- Line height generous enough for dark-room reading; avoid dense blocks.
- Do not truncate every title. Truncate only where overflow would break layout,
  and always provide the full title on the details screen.
- Numerals tabular where times and counts align.

## Spacing and layout

`--space-1` … `--space-12` on a single scale. Horizontal shelf gutters use one
consistent step. Section rhythm uses one of two steps, not arbitrary values.

- Edge-to-edge canvas; safe margins preserve artwork bleed.
- Vertical scroll contains horizontally scrolling shelves.
- Laptop: reduced hero height and card size, same hierarchy.
- Narrow window: shelves scroll, navigation collapses to labelled icons. Never
  become a cramped desktop dashboard.
- Full-screen supported; survives resolution and monitor changes.

## Radii, elevation, layers

- `--radius-card`, `--radius-sheet`, `--radius-pill` — three radii, no more.
- `--shadow-card`, `--shadow-overlay` — elevation communicates stacking, not
  decoration.
- `--z-nav`, `--z-overlay`, `--z-player`, `--z-toast` — explicit layers. No
  `z-index: 9999`.

## Focus

The most important token in the product.

- `--focus-ring-width` thick enough to identify at TV distance.
- `--focus-ring-offset` so the ring never merges with artwork edges.
- Focus is visible on bright and dark artwork, and remains visible when the last
  input was a mouse.
- Focused cards scale `1.03`–`1.06` with a shadow. **Scaling must not reflow
  neighbours** — transform only, never layout.
- Focus is never conveyed by colour alone.

## Motion

| Token | Use |
|---|---|
| `--motion-fast` | Focus change, hover (≈140 ms) |
| `--motion-base` | Overlay/panel open, controls fade (≈180 ms) |
| `--motion-slow` | Hero transition, large surface change (≈240 ms) |
| `--ease-out` / `--ease-in-out` | Two curves only |

Rules:

- Durations tuned after profiling, not guessed at in review.
- No parallax, no constant background animation, no expensive blur stacks.
- `prefers-reduced-motion: reduce` zeroes non-essential motion (transforms,
  parallax, overshoot) but never removes state changes.
- Nothing autoplays on focus: no trailers, no audio.

## Component inventory

From `Development.md` §7.4.10. Each is a token-consuming, data-agnostic unit:

`AppShell`, `PrimaryNavigation`, `PageHeader`, `HeroFeature`, `MediaShelf`,
`MediaCard`, `EpisodeCard`, `CollectionCard`, `ProgressBar`, `StatusPill`,
`Artwork`, `SkeletonCard`, `MediaDetails`, `SeasonPicker`, `EpisodeList`,
`PlayerControls`, `PlaybackOverlay`, `UpNextPrompt`, `SearchInput`, `FilterBar`,
`EmptyState`, `ConfirmDialog`, `Toast`, `FocusManager`.

Rules:

- No hard-coded titles, poster URLs or episode data in production components.
- Every card handles: loading, loaded, missing artwork, error.
- Every list handles: empty, loading, offline, error.
- Every icon-only control has an accessible name.

## Enforcement

- A CI check greps for colour/spacing literals outside the token files (P4-01).
- Contrast pairs are computed in a test, not eyeballed.
- The impeccable design-detector hooks run on UI edits and flag drift at Stop.
