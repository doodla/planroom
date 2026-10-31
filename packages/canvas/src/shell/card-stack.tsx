/**
 * The card stack (Decisions): one card face up, the pick already made. In M1 its choices are
 * labels, the pre-selected one marked, since `decide` is not a command yet and the pane picks a
 * card (DR-85: a surface offers only what works). With no open decision the face-up card is the
 * pane's next action (DR-54), one tap.
 */

import { sanitize } from "@planroom/ui";

import { CANVAS_STRINGS } from "@planroom/contract";
import { useCanvas } from "../use-canvas";
import { useNextAction } from "./next-action";

export function CardStack() {
  const pane = useCanvas((s) => s.pane);
  const next = useNextAction();
  if (pane === null) {
    return null;
  }
  const card = pane.decision;
  const behind = Math.min(pane.more_decisions, 2);
  return (
    <section
      className="cards"
      data-concept="decisions"
      aria-label={CANVAS_STRINGS.decisions}
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
              <ul className="card-choices">
                {card.choices.map((c) => (
                  <li
                    key={c.id}
                    className={
                      c.preselected ? "card-choice picked" : "card-choice"
                    }
                    {...(c.preselected ? { "aria-current": "true" } : {})}
                  >
                    {sanitize(c.label)}
                  </li>
                ))}
              </ul>
              <p className="card-in-pane">{CANVAS_STRINGS.card_in_pane}</p>
            </>
          ) : next !== null && next.onPress === null ? (
            <p className="card-in-pane">{next.label}</p>
          ) : next !== null && next.onPress !== null ? (
            <div className="card-choices">
              <button
                type="button"
                className="button primary"
                onClick={next.onPress}
              >
                {next.label}
              </button>
            </div>
          ) : null}
        </article>
      </div>
    </section>
  );
}
