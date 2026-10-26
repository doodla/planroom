/**
 * No screen is blank (design-language.md, "Empty states"): the felt with a block waiting on it,
 * from the Iris art, and the one next action as a button, or as a line when the action lives
 * outside the tab.
 */

import type { ReactNode } from "react";

export function EmptyState({
  art,
  line,
  action,
  concept,
}: {
  art: string;
  /** The engine's text, when it has more to say than the action's label. */
  line?: ReactNode;
  action?: { label: string; onPress: () => void };
  /** The concept a control-less empty state belongs to, when it fills a section. */
  concept?: string;
}) {
  return (
    <section className="empty" data-empty="true" data-concept={concept}>
      <img className="empty-art" src={art} alt="" decoding="async" />
      {line !== undefined && <p className="empty-line">{line}</p>}
      {action && (
        <button
          type="button"
          className="button primary"
          onClick={action.onPress}
        >
          {action.label}
        </button>
      )}
    </section>
  );
}
