/**
 * The evidence sheet (architecture.md, A14): a press on an edge asks `ask why` about its two
 * blocks and this sheet shows the engine's whole answer, since the question returns the edges both
 * ways between everything under the two blocks, of every relation. It draws the engine's summary,
 * each item's reason and its map handles as buttons, which run and close the sheet. It closes on
 * its Close button, on Escape, on a press on the felt (the map's) and after a handle runs. Every
 * control sits in the Levels concept; Confirm, Reject and Ask Claude have no leaf in M1, so their
 * place, a choice row below the items, draws nothing yet.
 */

import { CANVAS_STRINGS } from "@planroom/contract";
import { fill, sanitize } from "@planroom/ui";
import { useEffect } from "react";

import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { emptyWords } from "../views/engine-empty";
import { CloseIcon } from "./icons";

export function Evidence() {
  const evidence = useCanvas((s) => s.evidence);
  const nodes = useCanvas((s) => s.map?.nodes);
  const run = useCanvas((s) => s.run);
  const canRun = useCanvas((s) => s.canRun);
  const closeEvidence = useCanvas((s) => s.closeEvidence);
  const phone = useMedia(PHONE);
  const open = evidence !== null;
  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        closeEvidence();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, closeEvidence]);
  if (evidence === null) {
    return null;
  }
  // A block's name as the map draws it, else the last part of its ID.
  const nameOf = (id: string) =>
    sanitize(
      nodes?.find((n) => n.id === id)?.name ?? id.split(":").at(-1) ?? id,
    );
  const result = evidence.result;
  const empty = result?.empty ?? null;
  const words = empty === null ? null : emptyWords(empty);
  const truncated = evidence.warnings.filter((w) => w.code === "truncated");
  return (
    <section
      className={phone ? "evidence" : "evidence panel"}
      role="dialog"
      aria-label={CANVAS_STRINGS.evidence}
      data-concept="levels"
    >
      <header className="evidence-head">
        <h2 className="evidence-title">
          {sanitize(
            fill(CANVAS_STRINGS.evidence_between, {
              a: nameOf(evidence.from),
              b: nameOf(evidence.to),
            }),
          )}
        </h2>
        <button
          type="button"
          className="icon-button"
          aria-label={CANVAS_STRINGS.close}
          onClick={closeEvidence}
        >
          <CloseIcon />
        </button>
      </header>
      {evidence.state === "reading" || result === null ? (
        <p className="evidence-reading">
          {sanitize(CANVAS_STRINGS.evidence_reading)}
        </p>
      ) : (
        <>
          <p className="evidence-summary">{sanitize(result.summary)}</p>
          <ul className="evidence-items">
            {result.items.map((item, i) => (
              <li key={`${item.id}:${i}`} className="evidence-item">
                <p className="evidence-why">{sanitize(item.why)}</p>
                {item.handles
                  .filter((h) => canRun(h))
                  .map((h, j) => (
                    <button
                      key={j}
                      type="button"
                      className="button"
                      onClick={() => {
                        void run(h);
                        closeEvidence();
                      }}
                    >
                      {sanitize(h.label)}
                    </button>
                  ))}
              </li>
            ))}
          </ul>
          {empty !== null && words !== null && (
            <div className="evidence-empty">
              {words.line !== undefined && <p>{words.line}</p>}
              {canRun(empty.next) && (
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    void run(empty.next);
                    closeEvidence();
                  }}
                >
                  {words.label}
                </button>
              )}
            </div>
          )}
          {truncated.map((w, i) => (
            <p key={i} className="evidence-note">
              {sanitize(w.message)}
            </p>
          ))}
        </>
      )}
    </section>
  );
}
