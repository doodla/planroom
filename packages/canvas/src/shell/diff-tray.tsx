/**
 * The diff tray (Diffs): a badge with the count of open diffs that opens the tray, a list of the
 * engine's one-line summaries. An empty tray shows the tape art and the pane's next action.
 */

import { sanitize } from "@planroom/ui";

import emptyDiffs from "../assets/empty-diffs.webp";
import { CANVAS_STRINGS } from "@planroom/contract";
import { useCanvas } from "../use-canvas";
import { EmptyState } from "../views/empty-state";
import { useNextAction } from "./next-action";

export function TrayBadge({
  open,
  onPress,
}: {
  open: boolean;
  onPress: () => void;
}) {
  const count = useCanvas((s) => s.pane?.diffs.length ?? 0);
  return (
    <button
      type="button"
      className="button tray-badge"
      data-concept="diffs"
      aria-expanded={open}
      onClick={onPress}
    >
      <span>{CANVAS_STRINGS.diffs}</span>
      <span className="badge" aria-label={`${count}`}>
        {count}
      </span>
    </button>
  );
}

/** The closed drawer's one line: the top diff's summary. */
export function TrayPeek() {
  const first = useCanvas((s) => s.pane?.diffs[0]);
  return first ? (
    <span className="tray-peek">{sanitize(first.summary)}</span>
  ) : null;
}

export function TrayList() {
  const pane = useCanvas((s) => s.pane);
  const next = useNextAction();
  if (pane === null) {
    return null;
  }
  if (pane.diffs.length === 0) {
    return (
      <EmptyState
        art={emptyDiffs}
        concept="diffs"
        {...(next ? { action: next } : {})}
      />
    );
  }
  return (
    <ul className="tray-list" aria-label={CANVAS_STRINGS.diffs}>
      {pane.diffs.map((d, i) => (
        <li key={i} className="tray-row">
          <span className="tape" aria-hidden="true" />
          <span>{sanitize(d.summary)}</span>
        </li>
      ))}
    </ul>
  );
}
