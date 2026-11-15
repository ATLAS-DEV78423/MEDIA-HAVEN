# Navigation specification

Derived from `Development.md` §7.2 and §7.4.8. This is the highest-regression
area in the app: every phase must re-run the walkthrough at its checkpoint.

## Inputs the model must support

Keyboard arrows + Enter/Space/Escape · mouse · game controller · common TV
remote key events. Mouse is supported, never required. Hover never reveals
essential controls.

## Focus model

- Exactly one focused element at any time.
- Focus is always visible, including when the last input was a mouse.
- Focus order is deterministic: the same key sequence always produces the same
  result from the same state.
- Focus is never trapped inside a region the user cannot escape.

## Spatial rules

| Input | Behaviour |
|---|---|
| Left/Right inside a shelf | Move one card; scroll the shelf to keep focus visible |
| Left at the first card | Stay. Do **not** jump to an unrelated control |
| Right at the last card | Stay, or advance only where explicitly designed |
| Up/Down | Move between shelves preserving the nearest column |
| Up/Down with unequal shelves | Clamp to the last available card in the target shelf |
| Enter/Space | Activate the focused element |
| Escape/Back | Close the topmost overlay; only then navigate back |

"Preserve the nearest column" is the rule that makes shelves feel correct.
A shelf of 3 under a focused column 7 clamps to card 3, and returning restores
the intent where the design allows.

## Overlay stack

Modals, details surfaces and the player form a stack.

1. Escape always dismisses the **topmost** overlay.
2. The page beneath never receives focus while an overlay is open.
3. Closing an overlay restores focus to the exact element that opened it.
4. Deep-closing (Escape twice) unwinds one level per press.
5. Exiting the player restores focus to the originating card, not to the page
   header.

## Screen-level expectations

| Screen | Focus entry | Focus exit |
|---|---|---|
| Home | Navigation rail, then hero actions, then first shelf | Card → details |
| Movies/Shows | First card of the grid/shelf | Card → details |
| Details | Primary action (Play/Resume) | Back returns to originating card |
| Player | Controls hidden until input | Escape → originating card |
| Settings | First control of the active section | Back returns to previous page |
| Setup | First actionable control | Cancel exits cleanly |

## Player specifics

- Controls appear on any input; fade after a configurable idle period.
- The player is full-screen and distraction-free.
- Seeking reports actual completion — the UI must not show a seek as done while
  the engine is still seeking.
- Focus is visible over bright and dark video.
- No autoplay of the next episode until the user enables it.

## What breaks this

- A focus ring that disappears over pale artwork.
- Card scaling that reflows the row (`width`/`margin` animation instead of
  `transform`).
- Overlays that steal focus and never return it.
- Shelf edges that teleport focus to the navigation rail.
- Hover-only affordances for actions that have no keyboard path.

Each of these has a dedicated test in the P4-03 navigation suite.
