/**
 * The pane's next action as the card stack and the tray offer it, and the top lid's name. Once the
 * level a `map` handle opens is the one shown, offering it again would do nothing, so the press
 * becomes the way back up instead (ux-4).
 */

import { sanitize } from "@planroom/ui";

import { alreadyShown } from "../levels";
import { STRINGS } from "../strings";
import { useCanvas } from "../use-canvas";

/** The name on the top lid: the workspace, as the pane's breadcrumb names it. */
export function useTopName(): string {
  const name = useCanvas((s) => s.pane?.focus.breadcrumb[0]?.name);
  return name === undefined ? STRINGS.top : sanitize(name);
}

export interface NextPress {
  readonly label: string;
  readonly onPress: () => void;
}

/** The next action's press, the way back up once its level shows, or null when neither applies. */
export function useNextAction(): NextPress | null {
  const pane = useCanvas((s) => s.pane);
  const level = useCanvas((s) => s.level);
  const run = useCanvas((s) => s.run);
  const back = useCanvas((s) => s.back);
  const top = useTopName();
  if (pane === null) {
    return null;
  }
  const next = pane.next;
  if (alreadyShown(next, level)) {
    return level === null
      ? null
      : { label: STRINGS.backTo(top), onPress: () => void back() };
  }
  return { label: sanitize(next.label), onPress: () => void run(next) };
}
