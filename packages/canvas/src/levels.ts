/**
 * Where the person is on the levels, from the map's level and not the pane's (ux-4): the
 * breadcrumb's lids from the top down, and whether a handle's level is already the one shown.
 */

import type { Handle, Level } from "@planroom/contract";
import { LEVELS } from "@planroom/ui";

import { requestOf } from "./transport/request";

/** One lid of the breadcrumb: a level to go back up to, the top being `null`. */
export interface LevelCrumb {
  /** The level the lid shows; `null` is the top-level map, which `map` with no level draws. */
  readonly level: Level | null;
  readonly label: string;
}

/**
 * The breadcrumb at `level` (`null` for the top): the workspace's lid on top, then each level
 * down to this one. Intent, the plan, sits beside the system rather than inside it, so its only
 * lid above is the top.
 */
export function crumbsAt(level: Level | null, top: string): LevelCrumb[] {
  const first: LevelCrumb = { level: null, label: top };
  if (level === null) {
    return [first];
  }
  if (level === "intent") {
    return [first, { level, label: level }];
  }
  const from = LEVELS.indexOf("system") + 1;
  const to = LEVELS.indexOf(level) + 1;
  return [
    first,
    ...LEVELS.slice(from, to).map((l) => ({ level: l, label: l })),
  ];
}

/** The level a `map` handle asks for: its `--level`, or `null` for the top; not a map, undefined. */
export function handleLevel(handle: Handle): Level | null | undefined {
  if (handle.command !== "map") {
    return undefined;
  }
  const level = requestOf(handle).level;
  return typeof level === "string" &&
    (LEVELS as readonly string[]).includes(level)
    ? (level as Level)
    : null;
}

/** Whether `handle` would show the level already shown, so offering it again would do nothing. */
export function alreadyShown(handle: Handle, level: Level | null): boolean {
  const asked = handleLevel(handle);
  return asked !== undefined && asked === level;
}
