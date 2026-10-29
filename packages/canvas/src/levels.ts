/**
 * Where the person is on the levels, from the map's level and not the pane's (ux-4): the
 * breadcrumb's lids from the top down, and whether a handle's level is already the one shown.
 */

import type { Handle, Level, LevelVocab } from "@planroom/contract";
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

/**
 * The level a `map` handle asks for: its `--level` when `levels` holds it, else `null` for the
 * top; not a map, undefined.
 */
export function handleLevel(
  levels: readonly LevelVocab[],
  handle: Handle,
): Level | null | undefined {
  if (handle.command !== "map") {
    return undefined;
  }
  const level = requestOf(handle).level;
  return levels.find((l) => l.id === level)?.id ?? null;
}

/** Whether `handle` would show the level already shown, so offering it again would do nothing. */
export function alreadyShown(
  levels: readonly LevelVocab[],
  handle: Handle,
  level: Level | null,
): boolean {
  const asked = handleLevel(levels, handle);
  return asked !== undefined && asked === level;
}
