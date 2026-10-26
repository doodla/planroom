import type { PaneModel } from "@planroom/contract";
import { expect, test } from "vitest";

import focusReply from "../../../tests/golden/cli/focus.json";
import { INITIAL_ENGINE } from "./engine";
import { fallbackFor } from "./fallback";
import {
  bandRow,
  CHOICE_COLUMNS,
  cutHunk,
  DOCK,
  fallbackRows,
  paneRows,
  rowHeight,
  rowWidth,
  type Row,
} from "./layout";
import type { EngineState } from "./state-types";

const golden = (focusReply as unknown as { result: PaneModel }).result;

/**
 * The engine's pane after a fixture scan has no open decision, diffs or presence yet, so the
 * layout tests also lay a busy pane over it: a four-choice card, two diffs, drift and one agent.
 */
const busy: PaneModel = {
  ...golden,
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
    batch: null,
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
  drift: {
    summary: "≠ 2 intended rules broken · 1 machine drift",
    rules_broken: 2,
    machine_drift: 1,
  },
  who: [
    {
      actor: {
        kind: "agent",
        session: "1a2b3c4d",
        role: "implementer",
        key: null,
      },
      surface: "mod",
      doing: "worker-status-via-api",
      focus: null,
      seen_at: "2026-10-26T09:00:00Z",
    },
  ],
  minimap: { ...golden.minimap, blocks: golden.minimap.blocks.slice(0, 5) },
};

function height(rows: Row[]): number {
  return rows.reduce((sum, r) => sum + rowHeight(r), 0);
}

function widest(rows: Row[]): number {
  return Math.max(...rows.map(rowWidth));
}

function buttons(rows: Row[]) {
  return rows.flatMap((r) => (r.kind === "buttons" ? r.buttons : []));
}

test("rows_fit_at_49x34: every section fits the narrowest dock with a four-choice card", () => {
  expect(busy.decision?.choices).toHaveLength(4);
  const rows = paneRows(busy, DOCK.columns, DOCK.rows);
  expect(height(rows)).toBeLessThanOrEqual(DOCK.rows);
  expect(widest(rows)).toBeLessThanOrEqual(DOCK.columns);
  // No fixed choice label is cut.
  for (const c of busy.decision?.choices ?? []) {
    expect(c.label.length).toBeLessThanOrEqual(CHOICE_COLUMNS);
    expect(buttons(rows).some((b) => b.label === c.label)).toBe(true);
  }
  // The list keeps its room: every block of the busy shows.
  for (const block of busy.minimap.blocks) {
    expect(buttons(rows).some((b) => b.label.endsWith(block.name))).toBe(true);
  }
});

test("rows_fit_compact_at_80x24_and_100x30: inline panes keep the focus line, the card and the list", () => {
  for (const [columns, rows] of [
    [80, 8],
    [100, 10],
    [40, 34],
  ] as const) {
    const laid = paneRows(busy, columns, rows);
    expect(height(laid)).toBeLessThanOrEqual(rows);
    expect(widest(laid)).toBeLessThanOrEqual(columns);
    expect(laid.some((r) => r.key === "drift")).toBe(false);
    expect(laid.some((r) => r.key === "card-question")).toBe(true);
  }
});

test("keys: the card's choices take 1 to 4, rows take the row-key pool", () => {
  const rows = paneRows(busy, DOCK.columns, DOCK.rows);
  const hotkeys = buttons(rows).map((b) => b.hotkey);
  expect(hotkeys.slice(0, 4)).toEqual(["1", "2", "3", "4"]);
  expect(hotkeys.slice(4)).toEqual(["5", "6", "7", "8", "9", "0", "b"]);
  expect(new Set(hotkeys).size).toBe(hotkeys.length);
});

test("no_decision_shows_next: with no open decision the card is the next action", () => {
  const rows = paneRows({ ...golden, decision: null }, DOCK.columns, DOCK.rows);
  const next = buttons(rows).find((b) => b.key === "next");
  expect(next?.label).toBe(golden.next.label);
  expect(next?.action).toEqual({
    kind: "run",
    command: "map",
    args: ["--level", "container"],
    label: golden.next.label,
  });
});

test("a cut hunk keeps a header that counts its lines", () => {
  expect(
    cutHunk(["@@ -10,4 +10,5 @@ fn main", " a", "-b", "+c", "+d", " e"], 2),
  ).toBe("@@ -10,2 +10,1 @@ fn main\n a\n-b");
  expect(cutHunk(["not a header"], 2)).toBeNull();
  expect(cutHunk([], 2)).toBeNull();
});

const STATES: EngineState["status"][] = [
  "resolving",
  "fetching",
  "build_offered",
  "building",
  "failed",
  "version_mismatch",
  "person_key_mismatch",
  "unsupported_platform",
  "ready",
];

test("no_blank_pane: every fallback has a headline and at most one action, within the dock", () => {
  for (const status of STATES) {
    const engine: EngineState = {
      ...INITIAL_ENGINE,
      status,
      line: "A line the resolver wrote that runs well past the width of the dock",
    };
    const rows = fallbackRows(
      fallbackFor(engine, "workspace_not_found"),
      DOCK.columns,
    );
    expect(rows[0]?.kind).toBe("text");
    expect(widest(rows)).toBeLessThanOrEqual(DOCK.columns);
    expect(buttons(rows).length).toBe(
      status === "unsupported_platform" ? 0 : 1,
    );
  }
  const build = fallbackRows(
    fallbackFor({ ...INITIAL_ENGINE, status: "build_offered" }),
    DOCK.columns,
  );
  expect(buttons(build)[0]?.action).toEqual({ kind: "fallback", run: "build" });
});

test("band_next_is_undoable: the band presses only a leaf a stray digit can't hurt", () => {
  const map = bandRow({ model: golden }, 80);
  expect(map.kind === "buttons" && map.buttons[0]?.hotkey).toBe("1");
  // scan isn't undoable in the catalogue, so the band only names it.
  const scan = bandRow(
    {
      model: {
        ...golden,
        next: { command: "scan", args: ["."], label: "Map these 5 repos" },
      },
    },
    80,
  );
  expect(scan.kind).toBe("text");
  const unknown = bandRow(
    {
      model: {
        ...golden,
        next: { command: "handoff merge", args: ["k"], label: "Merge" },
      },
    },
    80,
  );
  expect(unknown.kind).toBe("text");
  const fetching = bandRow(
    { fallback: fallbackFor({ ...INITIAL_ENGINE, status: "fetching" }) },
    30,
  );
  expect(rowWidth(fetching)).toBeLessThanOrEqual(30);
});

test("a person key mismatch is a pane Button and never the band's", () => {
  const f = fallbackFor({ ...INITIAL_ENGINE, status: "person_key_mismatch" });
  expect(buttons(fallbackRows(f, DOCK.columns))[0]?.action).toEqual({
    kind: "fallback",
    run: "rekey",
  });
  expect(bandRow({ fallback: f }, 80).kind).toBe("text");
});
