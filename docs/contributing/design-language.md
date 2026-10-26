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

Everything else lives inside those five, so nothing adds a sixth [DR-77]:

- **Pairing** is a Decisions card. On an Artifact device the owner hasn't paired, or whose key the browser dropped, the face-up card is "Pair this device", with the code field on it. On the local canvas, while remote mode is on, the card stack holds a remote card, "Shared as an Artifact", whose choice "Pair a phone" shows the code. In the pane it is an entry of the remote picker.
- **Ask Claude** is a choice on a block's evidence sheet, under Levels, or on a card, under Decisions.
- **Signing in** has no control. A tab that needs a fresh link shows a full-page empty state with its one next action.
- **Pickers** in the pane are cards dealt face up, and each belongs to the idea it picks from: lenses to Lenses, projects and stages to Intent, hand-offs to Diffs, and the remote actions, which act on cards and edits, to Decisions.

## Palette

Colors are CSS variables in `packages/ui`, with a designed dark mode, not an inverted one. The table is felt, not paper: never a cream or off-white background. Panels stay in the felt family too, because the bottom sheet at half height covers half a phone screen.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--table` | `#cfe6dc` mint felt | `#10231d` pine felt | Page background |
| `--card` | `#e3f1ea` pale mint | `#17312a` | Panels, cards, and the bottom sheet's body |
| `--ink` | `#1b2a33` | `#e4f1ea` | Text |
| `--muted` | `#4c6170` | `#9db8ab` | Secondary text |
| `--chalk` | `#2d5a4a` deep felt green, a 2 px dash | `#e4f1ea` at 70% | Intended-layer outlines |
| `--block-service` | `#e8492b` tomato | `#ff7a5c` | Services |
| `--block-library` | `#2f5bd3` cobalt | `#7f9dff` | Libraries |
| `--block-store` | `#f2b632` sunflower | `#f5c95c` | Datastores and queues |
| `--block-infra` | `#6b8f71` sage | `#8fb596` | Tooling and observability |
| `--block-service-border`, `--block-library-border`, `--block-store-border`, `--block-infra-border` | `#bc2f14`, `#1f43a8`, `#835c08`, `#4f6953` | `#ffa48e`, `#aabdff`, `#f8dc96`, `#b4cfb8` | Block borders: a darker shade in light mode and a lighter one in dark mode, at least 4.5:1 on the felt [DR-77] |
| `--pass`, `--fail`, `--wait` | `#1a6b43`, `#b3261e`, `#6b5c0c` | `#5fd39a`, `#ff8a80`, `#e6cf6a` | Semantic states and diff tape, separate from block colors |

Block colors encode a Container-level kind's `tone`, which its pack declares as `service`, `library`, `store` or `infra`, one per block token above. A kind with no tone draws as `--card` with its label. Surfaces read the tone from the view model and never switch on a kind ID [DR-46]. Block colors are never the only signal: each block also carries its kind as a label or icon. Tape isn't told by color alone either: added tape carries `+`, removed tape `−` and changed tape `~`.

Lens yarn has no tokens yet. Until packs bring lenses in M1, every edge draws in `--muted`. M1 adds one yarn token per lens to this table, each held to the 3:1 mark bar below [DR-82].

Every token meets a contrast bar against `--table` and `--card` in both themes: 4.5:1 for text, and 3:1 for marks such as chalk outlines, tape and edges. A block's fill leans on its border and its label, so the border is the mark that counts. The bar checks the border tokens for blocks, since the fills alone measure as low as 1.39:1 on the light felt. `packages/ui/src/tokens.test.ts::contrast_meets_the_bar` checks every pair [DR-77]. White chalk on the light felt measured 1.24:1, which is why light mode draws dark chalk.

## Type

| Role | Face | Use |
|---|---|---|
| Display | Bricolage Grotesque, 600 and 800 | Block labels, headings, decision cards |
| Text | Atkinson Hyperlegible Next, 400 and 600 | Body, tables, evidence |
| Code | JetBrains Mono, 400 | Code, paths and IDs only, never as decoration |

All three are OFL-1.1. The canvas self-hosts them from `@fontsource` packages. The Artifact loads them from Google Fonts.

## Shape and depth

- Blocks are chunky: 12 px radius, a 2 px border in the tone's `--block-*-border` token, and a 3 px offset shadow below, like a toy brick.
- Panels use the same border and shadow. Buttons are rectangles with an 8 px radius, not pills.
- Depth means level: a block that holds others shows a lid edge.

## Motion

Three moments, each under 300 ms, each replaced by a crossfade under `prefers-reduced-motion`:

1. **Accepting a ghost.** The translucent block drops onto the felt and settles with one small bounce.
2. **A diff resolving.** The tape strip peels off the edge and the edge settles to its lens color.
3. **A level zoom.** The block's lid lifts and the view dives into it. Zooming out drops the lid back.

Nothing else animates beyond hover and focus feedback.

## Layout

At 1440 px the breadcrumb, the lens switcher and the project shelf run across the top. The zoom rail sits on the left edge and the card stack on the right. The diff tray and the plan board share a drawer along the bottom, and the map fills the rest.

## Touch

The phone layout at 390 px puts the breadcrumb, the lens switcher and the project shelf in one row at the top. The card stack, the diff tray and the plan board share a bottom sheet with two detents: peek, which shows the face-up card's question and its choices, and half, which shows the tray, the board or the whole card. The map fills the rest, and every target is at least 44 px.

The top row never wraps or shrinks a target. On the phone it holds four targets at most:

- the current level's lid, with search inside it;
- a `⋯` lid that opens the ancestors as a list, shown below the top level;
- the lens switcher as one swatch of the current lens, which opens a sheet of the lenses;
- the project shelf as one chip, which opens a sheet of the projects.

The swatch and the chip open a sheet only once there is something to pick. A picker with nothing to pick, such as a switcher with one lens or a shelf with no project to change to, isn't a button. It shows what it holds, flat on the felt. A project shelf with nothing to pick is a dashed chalk outline, whether or not it holds a project. Until M1 brings more lenses and `intent new`, both show what they hold [DR-86].

The current lid stacks over the last 10 px of the lid before it. So each ancestor lid, the `⋯` lid among them, is that much wider than a target, and the part a press reaches stays at least 44 px [DR-86].

`tests/e2e/surface.spec.ts::the_top_row_holds_four_targets_at_most_every_target_is_44px_and_nothing_overflows` checks the row at the top and one level down. It measures what a press reaches with `elementFromPoint`, since a box can't show what a neighbor paints over. `the_lens_switcher_and_the_project_shelf_offer_only_what_works` and `the_lens_switcher_and_the_project_shelf_offer_only_what_works_on_the_phone` check the pickers at both sizes. `tests/e2e/quality.spec.ts::top_row_fits_at_the_deepest_level`, planned for M1, will check the row at code level with several projects and every lens.

| Gesture | Does |
|---|---|
| Pinch, or drag with two fingers | zooms and pans the map geometrically; it never changes level |
| Drag with one finger on the felt | pans |
| Tap a block | selects and focuses it: the ego view of its neighbors |
| Double tap a block, or tap the open lid the breadcrumb shows for the selected block | dives one level into the block, with the lid-lift motion |
| Tap the `⋯` lid, then an ancestor in its list | goes back up to that level, the top-level map included |
| Long press a block or an edge | opens the bottom sheet with its evidence and the action tray (see [Edit by hand](#edit-by-hand)) |
| Tap a card's choice | picks it, in one tap |

`tests/e2e/quality.spec.ts::gestures_follow_the_gesture_table` drives each row at 390×844 with touch emulation.

## Edit by hand

You shape the map with your hands, the way you'd move blocks on a table. Each edit runs one catalogue `edit` leaf and shows as pending until the engine confirms it, with an Undo that runs `edit undo`.

| You do | On the desktop | On the phone | Runs |
|---|---|---|---|
| Rename, group with, hide or pin a block | the selected block's action tray, under the block | the action tray in the long-press sheet | `edit rename`, `edit group`, `edit hide`, `edit pin` |
| Move a block into another | drop it on the other block's lid | the tray's Move into, with the target picked from a list | `edit move` |
| Pin a block's position | drag it and drop it on empty felt | the tray's Pin, at its current place | `edit pin` |
| Make an intended relation | draw chalk: drag from the selected block's chalk handle to another block, with a dashed chalk line following the pointer | the tray's Relate to, with the target picked from a list | `edit relate --layer intended` |
| Forbid a relation | the relation's popover offers Forbid | the same popover, from a tap on the relation | `edit forbid` |

On the phone a one-finger drag always pans, so nothing moves a block by dragging there. The action tray belongs to Levels and the relation popover to Intent, so neither adds a top-level concept.

## Keyboard and screen readers

The canvas follows the pane's model, so a person who never touches a pointer can reach every block and action:

- Tab reaches the top-level controls in order, and every control shows a visible focus ring.
- On the map, the arrow keys move the selection between sibling blocks, Enter dives into the selected block, and Escape goes up a level.
- The selected block's action tray takes the focus with one Tab, and Enter or Space presses its buttons.
- A visually hidden outline of the `MapView` rows, from `@planroom/ui`, has `role="tree"`, so a screen reader announces each block with its kind, level and lifted edges.

`tests/e2e/quality.spec.ts::keyboard_reaches_every_block_and_action` drives the map with keys alone, and `axe_finds_no_serious_violation` runs `@axe-core/playwright` on every view.

## Empty states

No screen is blank. Every empty state shows the felt with one block waiting on it and the single next action as a button, for example "Scan this repo". The empty-state art comes from Iris, with its prompt beside it under `docs/art/`.

## Don't

- A cream or off-white background, or purple-on-white gradients.
- Italic accent words in headlines, or "01/02/03" section labels.
- Monospace text as decoration, pill buttons everywhere, or a generic three-card hero.
- Inter, Roboto, Arial, system-ui or Space Grotesk.
