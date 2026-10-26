/**
 * The card stack (Decisions): one card face up, the pick already made. With no open decision the
 * face-up card is the pane's next action (DR-54). Every choice is one tap.
 */

import { sanitize } from "@planroom/ui";

import { STRINGS } from "../strings";
import { useCanvas } from "../use-canvas";

export function CardStack() {
  const pane = useCanvas((s) => s.pane);
  const run = useCanvas((s) => s.run);
  if (pane === null) {
    return null;
  }
  const card = pane.decision;
  const behind = Math.min(pane.more_decisions, 2);
  return (
    <section
      className="cards"
      data-concept="decisions"
      aria-label={STRINGS.decisions}
    >
      <div className="card-stack" style={{ paddingBottom: behind * 6 }}>
        {Array.from({ length: behind }, (_, i) => (
          <div
            key={i}
            className="card card-behind"
            style={{
              transform: `translate(${(i + 1) * 4}px, ${(i + 1) * 6}px)`,
            }}
            aria-hidden="true"
          />
        ))}
        <article className="card face-up">
          {card ? (
            <>
              <h2 className="card-question">{sanitize(card.question)}</h2>
              <p className="card-why">{sanitize(card.why)}</p>
              <div className="card-choices">
                {card.choices.map((c) => (
                  <button
                    key={c.id}
                    type="button"
                    className={c.preselected ? "button primary" : "button"}
                    aria-pressed={c.preselected}
                    onClick={() =>
                      void run({
                        command: "decide",
                        args: [card.id, c.id],
                        label: c.label,
                      })
                    }
                  >
                    {sanitize(c.label)}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <div className="card-choices">
              <button
                type="button"
                className="button primary"
                onClick={() => void run(pane.next)}
              >
                {sanitize(pane.next.label)}
              </button>
            </div>
          )}
        </article>
      </div>
    </section>
  );
}
