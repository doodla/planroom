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
  hint,
  concept,
}: {
  art: string;
  /** The engine's text, when it has more to say than the action's label. */
  line?: ReactNode;
  action?: { label: string; onPress: () => void };
  /** An exact command that clears the problem, after the line; sanitized by the caller. */
  hint?: string;
  /**
   * The concept the state belongs to when it fills a section or holds a control, so every control
   * on the page sits in one of the five.
   */
  concept?: string;
}) {
  return (
    <section className="empty" data-empty="true" data-concept={concept}>
      <img className="empty-art" src={art} alt="" decoding="async" />
      {line !== undefined && <p className="empty-line">{line}</p>}
      {hint !== undefined && <code className="empty-hint">{hint}</code>}
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
