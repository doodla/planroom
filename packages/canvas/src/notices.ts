/**
 * What the engine said that the person should see in place (architecture.md, "The canvas"): a
 * refused command's message and hint, a press's warnings, and the shown view's own warnings.
 * Every line is engine-authored and sanitized; the same line shows once.
 */

import type { Warning } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

export interface Notice {
  readonly line: string;
  /** An exact command that clears the problem, when the engine named one. */
  readonly hint: string | null;
}

/**
 * The notices in the engine's order: the error, then the press's warnings, then the view's. A
 * warning's `message` is its line and has no hint. The first of equal lines stays.
 */
export function noticesOf(
  error: Notice | null,
  press: readonly Warning[],
  view: readonly Warning[],
): Notice[] {
  const all: Notice[] = [
    ...(error === null
      ? []
      : [
          {
            line: sanitize(error.line),
            hint: error.hint === null ? null : sanitize(error.hint),
          },
        ]),
    ...[...press, ...view].map((w) => ({
      line: sanitize(w.message),
      hint: null,
    })),
  ];
  const seen = new Set<string>();
  return all.filter((n) => !seen.has(n.line) && seen.add(n.line));
}

/** The first `max` notices to draw, and how many more there are. */
export function shownNotices(
  notices: readonly Notice[],
  max = 3,
): { shown: Notice[]; more: number } {
  return {
    shown: notices.slice(0, max),
    more: Math.max(0, notices.length - max),
  };
}
