/**
 * Where the person is on the levels, from the map's level and not the pane's (ux-4): the
 * breadcrumb's lids from the top down, and whether a handle's level is already the one shown.
 */

import type { Handle, Level, LevelVocab, NodeId } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

import { requestOf } from "./transport/request";

/** One lid of the breadcrumb: a level to go back up to, the top being `null`. */
export interface LevelCrumb {
  /** The level the lid shows; `null` is the top-level map, which `map` with no level draws. */
  readonly level: Level | null;
  readonly label: string;
}

/**
 * The breadcrumb at `level` (`null` for the top) on `levels`, the shown map's ladder: the
 * workspace's lid on top, then each chain level after the ladder's top down to this one, each
 * named by its label. A level apart from the chain, such as the plan's, sits beside it rather
 * than inside it, so its only lid above is the top.
 */
export function crumbsAt(
  levels: readonly LevelVocab[],
  level: Level | null,
  top: string,
): LevelCrumb[] {
  const first: LevelCrumb = { level: null, label: top };
  if (level === null) {
    return [first];
  }
  const entry = levels.find((l) => l.id === level);
  if (entry?.apart) {
    return [first, { level, label: sanitize(entry.label) }];
  }
  const chain = levels.filter((l) => !l.apart);
  const to = chain.findIndex((l) => l.id === level) + 1;
  return [
    first,
    ...chain
      .slice(1, to)
      .map((l) => ({ level: l.id, label: sanitize(l.label) })),
  ];
}

/** The top of the ladder: the first level that is not apart, or null before a map has drawn. */
export function topOf(levels: readonly LevelVocab[]): LevelVocab | null {
  return levels.find((l) => !l.apart) ?? null;
}

/**
 * Whether a view is the top-level map: nothing focused, and no level asked or the top's. `map`
 * and `map --level <the top>` are one view (DR-119).
 */
export function atTop(
  level: Level | null,
  focus: NodeId | null,
  top: Level,
): boolean {
  return focus === null && (level === null || level === top);
}

/** The parts of a view a `map` handle names; each is present only when the handle names it. */
export interface HandleView {
  /** The `--level` when `levels` holds it, else null for the top. */
  readonly level?: Level | null;
  readonly lens?: string;
  readonly focus?: NodeId;
}

/**
 * The view a `map` handle opens, from its `--level`, `--lens` and `--focus`; undefined for a
 * handle that is not a `map` (DR-119: every way to a view is the arguments of one `map` handle).
 */
export function viewOf(
  levels: readonly LevelVocab[],
  handle: Handle,
): HandleView | undefined {
  if (handle.command !== "map") {
    return undefined;
  }
  const asked = requestOf(handle);
  const view: { -readonly [K in keyof HandleView]: HandleView[K] } = {};
  if ("level" in asked) {
    view.level = levels.find((l) => l.id === asked.level)?.id ?? null;
  }
  if (typeof asked.lens === "string") {
    view.lens = asked.lens;
  }
  if (typeof asked.focus === "string") {
    view.focus = asked.focus;
  }
  return view;
}

/** The view on the canvas as the engine drew it: the shown map's `level`, `lens` and `focus`. */
export interface ShownView {
  readonly level: Level | null;
  readonly lens: string | null;
  readonly focus: NodeId | null;
}

/**
 * Whether `handle` would show the view already shown, so offering it again would do nothing: the
 * same level and focus, and the same lens when the handle names one. A handle that names no
 * level opens the top, or, with a focus, the level the engine picks below it.
 */
export function alreadyShown(
  levels: readonly LevelVocab[],
  handle: Handle,
  shown: ShownView,
): boolean {
  const asked = viewOf(levels, handle);
  if (asked === undefined) {
    return false;
  }
  const focus = asked.focus ?? null;
  if (focus !== shown.focus) {
    return false;
  }
  if (asked.lens !== undefined && asked.lens !== shown.lens) {
    return false;
  }
  if (asked.level === undefined && focus !== null) {
    return true;
  }
  const top = topOf(levels)?.id ?? null;
  return (asked.level ?? top) === (shown.level ?? top);
}

/** The request for the view the store holds: nulls left out, the top's level named for a lens. */
export interface MapRequest {
  readonly detail: "graph";
  readonly level?: Level;
  readonly lens?: string;
  readonly focus?: NodeId;
}

/**
 * The `map` request for an asked view. The focus is the trail's last block. At the top the level
 * is left out, unless a lens is picked: `map --lens X` alone takes the lens's own level, so a
 * lens picked at the top names the top.
 */
export function mapRequest(
  view: {
    readonly level: Level | null;
    readonly lens: string | null;
    readonly trail: readonly { readonly id: NodeId }[];
  },
  top: Level | null,
): MapRequest {
  const focus = view.trail.at(-1)?.id ?? null;
  const request: { -readonly [K in keyof MapRequest]: MapRequest[K] } = {
    detail: "graph",
  };
  let level = view.level;
  if (top !== null && atTop(view.level, focus, top)) {
    level = view.lens === null ? null : top;
  }
  if (level !== null) {
    request.level = level;
  }
  if (view.lens !== null) {
    request.lens = view.lens;
  }
  if (focus !== null) {
    request.focus = focus;
  }
  return request;
}

/**
 * The level a dive into `node` opens, or null when it opens nothing. From a chain level it is
 * the entry after the shown one; a level apart opens itself, since the engine would open the top
 * under one of its blocks. The block must hold something there (`counts`). The canvas keeps no
 * copy of the engine's `Ladder::below`.
 */
export function diveOf(
  levels: readonly LevelVocab[],
  shownLevel: Level,
  node: { readonly counts: Readonly<Record<string, number>> },
): Level | null {
  const at = levels.findIndex((l) => l.id === shownLevel);
  const here = levels[at];
  if (here === undefined) {
    return null;
  }
  const next = here.apart ? here : levels.slice(at + 1).find((l) => !l.apart);
  if (next === undefined || (node.counts[next.id] ?? 0) <= 0) {
    return null;
  }
  return next.id;
}
