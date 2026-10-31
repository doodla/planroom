/**
 * An engine empty state's words, as the canvas draws them: the line that says why there is nothing
 * here and what to do, and the short label of the one next action. Both are engine text, so both
 * pass the display-sanitizing pass. A label such as "Check again" says nothing on its own, so a
 * page that draws an empty state draws its line beside it.
 */

import { CANVAS_STRINGS, type Empty, type Handle } from "@planroom/contract";
import { fill, sanitize } from "@planroom/ui";

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

/**
 * Whether the top's empty state `empty` fills the page in place of the shell. It does, except
 * where the workspace holds a plan and no mapped repo: the shell stays there, so its project
 * shelf lists the plan and a pick lands on the project's own view.
 */
export function fillsThePage(empty: Empty): boolean {
  return empty.reason !== "plan_only";
}

/**
 * The commands whose answer the canvas draws: a view, a scan that redraws the map, a check again.
 * The server serves six (map, focus, scan, edit, ask and intent list); this is the subset whose
 * answer shows here. It is the one home of the rule that decides between a button and a line.
 */
export const DRAWN: ReadonlySet<string> = new Set(["map", "scan", "focus"]);

/** Whether pressing `handle` on the canvas ends with something drawn. */
export function drawsHere(handle: Handle): boolean {
  return DRAWN.has(handle.command);
}

/** A next action the canvas can't run, as a line that names the pane that can. */
export function inPane(handle: Handle): string {
  return fill(CANVAS_STRINGS.next_in_pane, { label: sanitize(handle.label) });
}
