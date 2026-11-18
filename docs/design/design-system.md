# Design system

The visual contract for `apps/desktop`. Every value used by the interface is a
custom property in `apps/desktop/src/styles/tokens.css`; components consume
tokens and contain no literals. `npm run check:tokens` enforces this.

The interface is a TV-first, liquid-glass surface: full-bleed artwork, floating
glass chrome, one neutral ink ramp, generous safe areas and a focus ring that
survives bright stills.

## Principles

1. **Artwork is the interface.** Surfaces stay quiet; stills carry the colour.
2. **One system, one scale.** No per-screen spacing inventions.
3. **Legible at three metres.** Type and targets are sized for a couch.
4. **Motion is a signal.** It shows focus and page change; it never runs
   unattended except the hero's paused-on-hover rotation.
5. **State is never colour alone.** Focus, selection and status always carry
   shape, text or an icon too.

## Colour

| Group | Tokens | Role |
|---|---|---|
| Ground | `--ink-void` … `--ink-500` | Pure black canvas, lifted panels |
| Ink | `--paper`, `--paper-90/72/54/38/22` | One neutral ramp for every text level |
| Accent | `--accent`, `--accent-soft`, `--accent-hover` | White primary action on black |
| Status | `--success`, `--danger`, `--warning` | Always paired with a label |
| Overlays | `--fill-*`, `--black-*`, `--scrim-*` | One alpha set for every translucent fill |
| Profile | `--tone-1` … `--tone-4` | Fallback avatar colours only; the backend stores and returns real hex values |
| Glass | `--glass-bg*`, `--glass-edge*`, `--glass-blur*` | The liquid-glass recipe |
| Focus | `--paper`, `--ring-w`, `--ring-offset` | Ring is always paper-white |

Rules:

- Normal text meets WCAG AA against its rendered surface; secondary ink is
  `--paper-54` or lighter.
- Gradients exist to protect copy over unknown artwork, never as decoration.
- The accent is white. Do not add a second accent colour.

## Typography

| Token | Use |
|---|---|
| `--font-display` (Space Grotesk) | Hero, page titles, section headings |
| `--font-condensed` (Roboto Condensed) | Wordmark and condensed labels |
| `--text-hero` … `--text-micro` | One responsive ramp, TV-scaled |
| `--tracking-display/title/tight/wide/wordmark` | Untracked body, tight display |
| `--leading-hero/title/body/copy` | Four line-heights only |
| `--measure-copy`, `--measure-hero` | Line-length caps |

Rules: tabular numerals for time and counts; no essential text below
`--text-micro`; truncation only where overflow would break layout.

## Spacing and layout

- `--s-0` … `--s-12` is the only spacing scale.
- `--safe-x`, `--safe-y`, `--safe-top` are the TV safe areas (80 pt sides at
  large sizes, collapsing on laptops).
- `--card-w`, `--card-w-wide`, `--card-gap`, `--row-gap` govern shelves; grid
  columns use `em` breakpoints, never device widths.
- `--hero-min-h`, `--backdrop-h`, `--sheet-lift`, `--sheet-pad` shape the
  cinematic surfaces.
- Vertical scroll contains horizontal shelves. Narrow windows keep the same
  hierarchy rather than becoming a dashboard.

## Radii, glass, layers

- Three radii: `--r-sm`, `--r-card`, `--r-lg`/`--r-sheet`, and `--r-pill`.
- `.glass` in `base.css` is the shared glass surface: layered gradient, edge,
  inner specular line, blur and float shadow.
- Layers are named (`--z-nav`, `--z-sheet`, `--z-player`, `--z-dialog`,
  `--z-toast`). No numeric `z-index`.

## Focus

- `useSpatialFocus` moves real DOM focus by geometry, so Enter/Space and screen
  readers work without a parallel state machine.
- `--ring-w` and `--ring-offset` keep the ring clear of artwork edges.
- Focused cards scale with `transform` only; neighbours never reflow.
- Arrow keys are owned by the player while it is open (`data-player="active"`
  on the root element).

## Motion

| Token | Use |
|---|---|
| `--t-fast` | Hover and focus colour |
| `--t-base` | Controls, chips, chrome fade |
| `--t-slow` | Sheets and dialogs |
| `--t-reveal` | Page rise on entry |
| `--ease-out`, `--ease-in-out`, `--ease-spring` | Three curves only |

`prefers-reduced-motion: reduce` and the in-app Reduce motion preference zero
non-essential motion (page rise, hero drift, spinner) without removing state.

## Component inventory

`Nav` · `Hero` · `Shelf` · `MediaCard` · `ActionButton` · `Chip` ·
`ProgressBar` · `Toggle` · `Segmented` · `Icon` (authored SVG set) ·
`PageHeader` · `Panel` · `EmptyState` · `FieldRow` · `CategoryTile` ·
`BrowseView` · `Toast` (in `App`) · `PlayerPage` chrome.

Every card handles missing artwork (deterministic local still), every list
handles empty, and every backend call handles failure with a retry or an
explicit message. Controls are token-consuming and data-agnostic.

## Data rules

- No fixture catalogue, no mock transport. Screens read the `mediahaven-core`
  API through `src/services/api.ts`; `src/state/AppState.tsx` holds only the
  fetched snapshot and local UI preferences.
- Artwork falls back to `public/artwork` until a metadata provider supplies
  posters (ADR-0011). The fallback is deterministic per title.
- No invented ratings, match scores or taglines: fields the backend does not
  return are hidden.

## Enforcement

- `node scripts/check-tokens.mjs` rejects hex/rgba/hsl and bare `px` in
  `src/**` outside `styles/tokens.css` (53 files, 0 violations at the time of
  writing).
- `npm run typecheck`, `npm run lint`, `npm run test` and `npm run build` gate
  the UI. Rust gates are `cargo test`, `cargo clippy` and `cargo fmt --check`.
