/**
 * What the engine said in place (architecture.md, "The canvas"): a refused command's message and
 * its hint, a press's warnings and the shown view's, over the map and never instead of it. The
 * list holds no control, so it belongs to no concept; it is a status the screen reader announces.
 */

import { fill } from "@planroom/ui";

import { CANVAS_STRINGS } from "@planroom/contract";
import { shownNotices } from "../notices";
import { useCanvas } from "../use-canvas";

export function Notices() {
  const notices = useCanvas((s) => s.notices);
  if (notices.length === 0) {
    return null;
  }
  const { shown, more } = shownNotices(notices);
  return (
    <ul
      className="notices"
      role="status"
      aria-live="polite"
      aria-label={CANVAS_STRINGS.notices}
    >
      {shown.map((n) => (
        <li key={n.line}>
          {n.line}
          {n.hint !== null && <code className="empty-hint">{n.hint}</code>}
        </li>
      ))}
      {more > 0 && (
        <li>{fill(CANVAS_STRINGS.more_notices, { count: String(more) })}</li>
      )}
    </ul>
  );
}
