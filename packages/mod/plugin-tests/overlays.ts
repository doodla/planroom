/**
 * Test-only overlays on the engine's golden `focus.json`, for pane sections the engine can't fill
 * yet (DR-82): each names the milestone that makes its state real and why, and goes when the
 * engine produces that state. `host.ts` lays them over the golden for `claude plugin test`, and
 * `src/layout.test.ts` lays the same consts over it for vitest, so both share one copy. Pure:
 * types only, and no golden read here; nothing under `tests/golden/` is ever hand-edited.
 */

import type { PaneModel } from "@planroom/contract";

/** A patch on the golden pane, named for the milestone that makes it real. */
export interface Overlay {
  milestone: "M1" | "M2";
  why: string;
  patch: Partial<PaneModel>;
}

/** The same card, with its batch, and the same two diff lines as `tests/e2e/overlays/m2-decision-and-diffs.focus.json`. */
export const M2_DECISION_AND_DIFFS: Overlay = {
  milestone: "M2",
  why: "The engine opens no decision card and finds no diffs yet: the decision runner and the diff engine are M2. Until then the pane's card and diff lines draw this one face-up card and two diffs.",
  patch: {
    decision: {
      id: "card_01JB7Q",
      question: "Apply Claude's 2 edits to the worker group?",
      why: "They move consumer.py under the queue it reads.",
      choices: [
        { id: "accept", label: "Accept both", preselected: true },
        { id: "first", label: "Only the first", preselected: false },
        { id: "reject", label: "Reject", preselected: false },
        { id: "later", label: "Later", preselected: false },
      ],
      stakes: "low",
      batch: {
        count: 2,
        items: [
          { command: "show", args: ["ev_01JB7Q1"], label: "Move consumer.py" },
          { command: "show", args: ["ev_01JB7Q2"], label: "Rename the group" },
        ],
      },
      person_only: true,
    },
    more_decisions: 1,
    diffs: [
      {
        summary: "worker → platform: + calls orders-api",
        open: { command: "diff", args: ["--item", "d1"], label: "Open" },
        hunk: ["@@ -1,2 +1,3 @@", " worker", "+  calls orders-api"],
      },
      {
        summary: "storefront: ~ checkout moved to cart",
        open: { command: "diff", args: ["--item", "d2"], label: "Open" },
        hunk: [],
      },
    ],
  },
};

/** Two intended rules broken and one machine drift, with a top item for a row key to select. */
export const M2_DRIFT: Overlay = {
  milestone: "M2",
  why: "The engine computes no drift yet: intended rules and machine drift are M2. Until then the drift line draws these counts and a top item, so its row key stays covered.",
  patch: {
    drift: {
      summary: "≠ 2 intended rules broken · 1 machine drift",
      rules_broken: 2,
      machine_drift: 1,
      top: { command: "ask", args: ["drift"], label: "Show the drift" },
    },
  },
};

/** One agent at work, as presence through `watch` will report it. */
export const M1_PRESENCE: Overlay = {
  milestone: "M1",
  why: "A pane has a who line only after a watch has written its session's row, and focus.json's run comes before any watch in the golden runs, so its pane has nobody in it. The who line draws one implementer at work here, so its row key stays covered.",
  patch: {
    who: [
      {
        actor: {
          kind: "agent",
          session: "1a2b3c4d",
          role: "implementer",
          key: null,
        },
        surface: "mod",
        client_id: "mod:1a2b3c4d",
        doing: "worker-status-via-api",
        focus: null,
        seen_at: "2026-10-26T09:00:00Z",
      },
    ],
  },
};

/** A next action that is a hand edit, which the band can press and then offer to undo. */
export const M1_EDIT_NEXT: Overlay = {
  milestone: "M1",
  why: "The fixture's pane offers a read as its next action; a pack suggestion that names an edit arrives with the plan's next step. Until then the next action is this rename, so a band press that can be undone stays covered.",
  patch: {
    next: {
      command: "edit",
      args: ["rename", "worker:repo:worker", "Workers"],
      label: "Rename worker to Workers",
    },
  },
};

/** `base` with each overlay's patch laid over it, in order. */
export function overlay(base: PaneModel, ...overlays: Overlay[]): PaneModel {
  return overlays.reduce<PaneModel>(
    (pane, o) => ({ ...pane, ...o.patch }),
    base,
  );
}
