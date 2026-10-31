/**
 * The pane's next action as the card stack and the tray offer it, and the top lid's name. Once the
 * level a `map` handle opens is the one shown, offering it again would do nothing, so the press
 * becomes the way back up instead (ux-4).
 */

import { fill, sanitize } from "@planroom/ui";

import { alreadyShown, atTop, topOf } from "../levels";
import { drawsHere, inPane } from "../views/engine-empty";
import { CANVAS_STRINGS } from "@planroom/contract";
import { useCanvas } from "../use-canvas";

/**
 * The name on the top lid: the workspace, as the pane's breadcrumb names it; before the pane
 * names it, the ladder's top level, as the shown map names it.
 */
export function useTopName(): string {
  const name = useCanvas((s) => s.pane?.focus.breadcrumb[0]?.name);
  const topLevel = useCanvas(
    (s) => s.map?.levels?.find((l) => !l.apart)?.label,
  );
  return sanitize(name ?? topLevel ?? CANVAS_STRINGS.levels);
}

export interface NextPress {
  readonly label: string;
  /** Null when the canvas can't run the action: the label then names the pane that can. */
  readonly onPress: (() => void) | null;
}

/** The next action's press, the way back up once its level shows, or null when neither applies. */
export function useNextAction(): NextPress | null {
  const pane = useCanvas((s) => s.pane);
  const map = useCanvas((s) => s.map);
  const run = useCanvas((s) => s.run);
  const canRun = useCanvas((s) => s.canRun);
  const back = useCanvas((s) => s.back);
  const top = useTopName();
  if (pane === null) {
    return null;
  }
  const next = pane.next;
  const levels = map?.levels ?? [];
  // Where you are is the shown map's view, not the asked one (the asked view counts only while a
  // request state stands).
  const shown = {
    level: map?.level ?? null,
    lens: map?.lens ?? null,
    focus: map?.focus ?? null,
  };
  if (alreadyShown(levels, next, shown)) {
    return atTop(shown.level, shown.focus, topOf(levels)?.id ?? "")
      ? null
      : {
          label: fill(CANVAS_STRINGS.back_to, { place: top }),
          onPress: () => void back(),
        };
  }
  if (!drawsHere(next) || !canRun(next)) {
    return { label: inPane(next), onPress: null };
  }
  return { label: sanitize(next.label), onPress: () => void run(next) };
}
