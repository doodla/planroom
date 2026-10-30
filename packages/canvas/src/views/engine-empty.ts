/**
 * An engine empty state's words, as the canvas draws them: the line that says why there is nothing
 * here and what to do, and the short label of the one next action. Both are engine text, so both
 * pass the display-sanitizing pass. A label such as "Check again" says nothing on its own, so a
 * page that draws an empty state draws its line beside it.
 */

import type { Empty } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

export interface EmptyWords {
  /** The engine's line, absent when it sends none. */
  readonly line: string | undefined;
  /** The next action's label. */
  readonly label: string;
}

export function emptyWords(empty: Empty): EmptyWords {
  return {
    line:
      empty.line === undefined || empty.line === null
        ? undefined
        : sanitize(empty.line),
    label: sanitize(empty.next.label),
  };
}

/**
 * The reasons the engine gives for an empty map when no scan can have made one yet: the
 * workspace's repos aren't mapped, or can't be, so nothing is planned there either.
 */
const BEFORE_A_SCAN: ReadonlySet<string> = new Set([
  "not_scanned",
  "no_repos",
  "repos_unborn",
  "repos_unreadable",
]);

/** Whether `empty` says the workspace has no scan to read yet. */
export function beforeAScan(empty: Empty | null | undefined): boolean {
  return empty != null && BEFORE_A_SCAN.has(empty.reason);
}
