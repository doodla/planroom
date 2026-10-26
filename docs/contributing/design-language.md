# Design language

This page defines Planroom's look and feel: the one concept that the canvas, the mod and the art share. Read it before you design a screen, a pane or an image.

## The concept: a planning table

Planroom is a tabletop where your system is laid out as toy blocks. You look down at it, pick blocks up, open them and move them around. Every visual element maps to one of the five ideas in R14:

| Idea | On the table | Canvas | Mod (terminal) |
|---|---|---|---|
| Levels | Blocks nest like boxes: lift a block's lid to see the blocks inside. | Zoom dives into a block; the breadcrumb is a row of stacked lids. | Indented block glyphs (`▣`) with the level name in the focus header. |
| Lenses | Colored yarn strung between pegs, one color per relationship kind. | Edge color and style per lens; one lens on by default. | Colored arrows in the mini-map; the lens name in the focus header. |
| Intent | Chalk outlines drawn on the felt where blocks should sit. Plans are index cards pinned beside the table. | Intended layer drawn as dashed chalk outlines; drift is a block outside its outline. | `intended` and `actual` columns in diffs; drift marked with `≠`. |
| Diffs | Ghost blocks (translucent, proposed) and strips of tape (green added, red removed, amber changed). | Ghosts render translucent until accepted; tape marks on changed edges. | `Code format="diff"` previews; ghosts listed with a dim style. |
| Decisions | A card dealt to you, with the pick already face up. | A card with the pre-selected choice and one-tap buttons. | The decision card in the pane, choices on hotkeys. |

## Palette

Colors are CSS variables in `packages/ui`, with a designed dark mode, not an inverted one. The table is felt, not paper: never a cream or off-white background.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--table` | `#cfe6dc` mint felt | `#10231d` pine felt | Page background |
| `--card` | `#f6fbf8` | `#17312a` | Panels, cards |
| `--ink` | `#1b2a33` | `#e4f1ea` | Text |
| `--muted` | `#4c6170` | `#9db8ab` | Secondary text |
| `--chalk` | `#2d5a4a` deep felt green, a 2 px dash | `#e4f1ea` at 70% | Intended-layer outlines |
| `--block-service` | `#e8492b` tomato | `#ff7a5c` | Services |
| `--block-library` | `#2f5bd3` cobalt | `#7f9dff` | Libraries |
| `--block-store` | `#f2b632` sunflower | `#f5c95c` | Datastores and queues |
| `--block-infra` | `#6b8f71` sage | `#8fb596` | Tooling and observability |
| `--pass`, `--fail`, `--wait` | `#1a6b43`, `#b3261e`, `#6b5c0c` | `#5fd39a`, `#ff8a80`, `#e6cf6a` | Semantic states and diff tape, separate from block colors |

Block colors encode container kind and are never the only signal: each block also carries its kind as a label or icon. Tape isn't told by color alone either: added tape carries `+`, removed tape `−` and changed tape `~`.

Every token meets a contrast bar against `--table` and `--card` in both themes: 4.5:1 for text, and 3:1 for marks such as chalk outlines, tape and edges. A block's fill leans on its darker border and its label, so the border is the mark that counts. `packages/ui/src/tokens.test.ts::contrast_meets_the_bar` checks every pair. White chalk on the light felt measured 1.24:1, which is why light mode draws dark chalk.

## Type

| Role | Face | Use |
|---|---|---|
| Display | Bricolage Grotesque, 600 and 800 | Block labels, headings, decision cards |
| Text | Atkinson Hyperlegible Next, 400 and 600 | Body, tables, evidence |
| Code | JetBrains Mono, 400 | Code, paths and IDs only, never as decoration |

All three are OFL-1.1. The canvas self-hosts them from `@fontsource` packages. The Artifact loads them from Google Fonts.

## Shape and depth

- Blocks are chunky: 12 px radius, a 2 px border in a darker shade of the block color, and a 3 px offset shadow below, like a toy brick.
- Panels use the same border and shadow. Buttons are rectangles with an 8 px radius, not pills.
- Depth means level: a block that holds others shows a lid edge.

## Motion

Three moments, each under 300 ms, each replaced by a crossfade under `prefers-reduced-motion`:

1. **Accepting a ghost.** The translucent block drops onto the felt and settles with one small bounce.
2. **A diff resolving.** The tape strip peels off the edge and the edge settles to its lens color.
3. **A level zoom.** The block's lid lifts and the view dives into it. Zooming out drops the lid back.

Nothing else animates beyond hover and focus feedback.

## Touch

The phone layout at 390 px puts the breadcrumb, the lens switcher and the project shelf in one row at the top. The card stack, the diff tray and the plan board share a bottom sheet with two detents: peek, which shows the face-up card's question and its choices, and half, which shows the tray, the board or the whole card. The map fills the rest, and every target is at least 44 px.

| Gesture | Does |
|---|---|
| Pinch, or drag with two fingers | zooms and pans the map geometrically; it never changes level |
| Drag with one finger on the felt | pans |
| Tap a block | selects and focuses it: the ego view of its neighbors |
| Double tap a block, or tap the open lid the breadcrumb shows for the selected block | dives one level into the block, with the lid-lift motion |
| Tap an ancestor's lid in the breadcrumb | goes back up to that level |
| Long press a block or an edge | opens its evidence in the bottom sheet |
| Tap a card's choice | picks it, in one tap |

`tests/e2e/quality.spec.ts::gestures_follow_the_gesture_table` drives each row at 390×844 with touch emulation.

## Empty states

No screen is blank. Every empty state shows the felt with one block waiting on it and the single next action as a button, for example "Scan this repo". The empty-state art comes from Iris, with its prompt beside it under `docs/art/`.

## Don't

- A cream or off-white background, or purple-on-white gradients.
- Italic accent words in headlines, or "01/02/03" section labels.
- Monospace text as decoration, pill buttons everywhere, or a generic three-card hero.
- Inter, Roboto, Arial, system-ui or Space Grotesk.
