/**
 * "Your edits" (Diffs, DR-68): this tab's hand edits on the workspace, newest first, each with
 * its line, its state (sending, done, or a conflict whose card the pane shows) and, once the
 * engine has written it, an Undo that sends the edit's own undo handle. `EditPeek` is the newest
 * row alone, so the closed drawer and the phone's peek keep Undo one press away. Both draw
 * nothing with no edits, so an empty tray stays as it was.
 */

import { CANVAS_STRINGS } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

import type { EditEntry } from "../store";
import { useCanvas } from "../use-canvas";

const STATE_TEXT: Record<EditEntry["state"], string> = {
  sending: CANVAS_STRINGS.sending,
  done: CANVAS_STRINGS.landed,
  conflict: CANVAS_STRINGS.conflict,
};

function Row({ entry }: { entry: EditEntry }) {
  const undo = useCanvas((s) => s.undo);
  return (
    <li className="edit-row" data-state={entry.state}>
      <span className="edit-line">{sanitize(entry.line)}</span>
      <span className="edit-state">{STATE_TEXT[entry.state]}</span>
      {entry.state === "done" && entry.undo !== null && (
        <button
          type="button"
          className="button"
          onClick={() => void undo(entry)}
        >
          {CANVAS_STRINGS.undo}
        </button>
      )}
    </li>
  );
}

export function EditList() {
  const edits = useCanvas((s) => s.edits);
  if (edits.length === 0) {
    return null;
  }
  return (
    <section className="edit-list" data-concept="diffs">
      <h2 className="edit-heading">{CANVAS_STRINGS.your_edits}</h2>
      <ul className="edit-rows">
        {edits.map((entry) => (
          <Row key={entry.idem} entry={entry} />
        ))}
      </ul>
    </section>
  );
}

export function EditPeek() {
  const newest = useCanvas((s) => s.edits[0]);
  if (newest === undefined) {
    return null;
  }
  return (
    <ul
      className="edit-peek"
      data-concept="diffs"
      aria-label={CANVAS_STRINGS.your_edits}
    >
      <Row entry={newest} />
    </ul>
  );
}
