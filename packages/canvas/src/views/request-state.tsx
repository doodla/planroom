/**
 * The request state (architecture.md, "The canvas"; DR-56, DR-77): on the Artifact, a view the
 * snapshot doesn't hold shows the engine-authored "Not in this snapshot" line, whose button writes
 * the read request; then "Asked · it arrives at the next sync" until the answer draws the view.
 * "Back to the map" stays on every state, and nothing here names `/room open`.
 */

import { sanitize } from "@planroom/ui";

import emptyMap from "../assets/empty-map.webp";
import { CANVAS_STRINGS } from "@planroom/contract";
import { useCanvas } from "../use-canvas";

export function RequestState() {
  const request = useCanvas((s) => s.request);
  const ask = useCanvas((s) => s.ask);
  const back = useCanvas((s) => s.back);
  if (request === null) {
    return null;
  }
  const askable = request.state === "unavailable" && request.canAsk;
  return (
    <section
      className="empty request-state"
      data-empty="true"
      data-concept="levels"
      data-request={request.state}
      aria-live="polite"
    >
      <img className="empty-art" src={emptyMap} alt="" decoding="async" />
      {askable ? (
        <button
          type="button"
          className="button primary"
          onClick={() => void ask()}
        >
          {sanitize(request.line)}
        </button>
      ) : (
        <p className="empty-line">{sanitize(request.line)}</p>
      )}
      {request.problem !== null && (
        <p className="empty-line">{sanitize(request.problem)}</p>
      )}
      <button type="button" className="button" onClick={() => void back()}>
        {CANVAS_STRINGS.back_to_map}
      </button>
    </section>
  );
}

/** The transport's note, such as a page with no live store, over the stage. */
export function TransportNote() {
  const note = useCanvas((s) => s.note);
  if (note === null) {
    return null;
  }
  return (
    <p className="transport-note" role="status">
      {sanitize(note)}
    </p>
  );
}
