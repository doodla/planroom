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
