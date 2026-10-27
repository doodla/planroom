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
import type { Shown } from "./room";
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

function texts(rows: Row[]): string[] {
  return rows.flatMap((r) =>
    r.kind === "text"
      ? [r.text]
      : r.kind === "buttons"
        ? r.buttons.map((b) => b.label)
        : [],
  );
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
    expect(texts(rows).some((t) => t.endsWith(block.name))).toBe(true);
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

test("keys: the card's choices take 1 to 4, rows an action targets take the row-key pool", () => {
  const rows = paneRows(busy, DOCK.columns, DOCK.rows);
  const hotkeys = buttons(rows).map((b) => b.hotkey);
  expect(hotkeys.slice(0, 4)).toEqual(["1", "2", "3", "4"]);
  // The two diffs; blocks run nothing until `map --focus` exists, so they take no key (ux-1).
  expect(hotkeys.slice(4)).toEqual(["5", "6"]);
  expect(new Set(hotkeys).size).toBe(hotkeys.length);
});

test("ux-1: no Button runs map --focus, which the engine refuses until M1", () => {
  for (const model of [golden, busy]) {
    for (const [columns, rows] of [
      [DOCK.columns, DOCK.rows],
      [80, 8],
    ] as const) {
      for (const b of buttons(paneRows(model, columns, rows))) {
        if (b.action.kind === "run") {
          expect(b.action.args, b.key).not.toContain("--focus");
        }
      }
    }
  }
  // The blocks still show, as plain rows.
  const rows = paneRows(golden, DOCK.columns, DOCK.rows);
  expect(
    rows.some((r) => r.kind === "text" && r.key === "block-worker:repo:worker"),
  ).toBe(true);
});

/** What pressing the golden pane's "Open the containers" showed. */
const containers: Shown = {
  command: "map",
  args: ["--level", "container"],
  seq: golden.focus.seq,
  line: "12 blocks at the container level",
  level: "container",
  blocks: ["kit/lib", "orders/api", "worker/consumer"],
};

test("ux-1: a pressed next action shows its result, not the same press again", () => {
  const rows = paneRows(golden, DOCK.columns, DOCK.rows, containers);
  expect(buttons(rows).some((b) => b.key === "next")).toBe(false);
  expect(rows.find((r) => r.key === "card-result")).toMatchObject({
    text: "12 blocks at the container level",
  });
  // The list shows the level it opened, and a press puts the pane's own level back.
  expect(rows.find((r) => r.key === "blocks-title")).toMatchObject({
    text: "Blocks · container",
  });
  expect(texts(rows)).toContain("□ worker/consumer");
  const back = buttons(rows).find((b) => b.key === "back");
  expect(back?.action).toEqual({ kind: "clear" });
  expect(back?.label).toBe("Back to the system level");
  expect(height(rows)).toBeLessThanOrEqual(DOCK.rows);
  expect(widest(rows)).toBeLessThanOrEqual(DOCK.columns);
  // The band says what it showed instead of offering the press.
  const band = bandRow({ model: golden }, 80, containers);
  expect(band.kind).toBe("text");
  expect(band.kind === "text" && band.text).toContain(
    "12 blocks at the container level",
  );
  // A newer pane, or another next action, offers the press again.
  for (const stale of [
    { ...containers, seq: golden.focus.seq - 1 },
    { ...containers, args: ["--level", "component"] },
  ]) {
    const again = paneRows(golden, DOCK.columns, DOCK.rows, stale);
    expect(buttons(again).some((b) => b.key === "next")).toBe(true);
    expect(bandRow({ model: golden }, 80, stale).kind).toBe("buttons");
  }
  // Compact panes fit too.
  const compact = paneRows(golden, 80, 8, containers);
  expect(height(compact)).toBeLessThanOrEqual(8);
});

test("security-5: an OSC 52 or RLO in a repo name never reaches the pane or the band raw", () => {
  const evil = "evil\u001b]52;c;aGVsbG8=\u0007name\u202eabc";
  const model: PaneModel = {
    ...busy,
    focus: {
      ...busy.focus,
      breadcrumb: [{ id: "ws:system:x", name: evil }],
    },
    next: { ...busy.next, label: evil },
    drift: { ...busy.drift, summary: evil },
    minimap: {
      ...busy.minimap,
      blocks: [{ id: "x:repo:x", name: evil, focused: false }],
    },
  };
  const shown: Shown = { ...containers, line: evil, blocks: [evil] };
  const all = [
    ...paneRows(model, DOCK.columns, DOCK.rows),
    ...paneRows({ ...model, decision: null }, DOCK.columns, DOCK.rows),
    ...paneRows({ ...model, decision: null }, DOCK.columns, DOCK.rows, {
      ...shown,
      seq: model.focus.seq,
    }),
    bandRow({ model: { ...model, decision: null } }, 80),
    bandRow({ model: { ...model, decision: null } }, 80, {
      ...shown,
      seq: model.focus.seq,
    }),
    ...fallbackRows(
      fallbackFor({ ...INITIAL_ENGINE, status: "failed", line: evil }),
      DOCK.columns,
    ),
  ];
  const drawn = texts(all).join("\n");
  expect(drawn).toContain("\\x1b]52");
  expect(drawn).toContain("⟨U+202E⟩");
  expect(hasRaw(drawn)).toBe(false);
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

test("an_empty_pane_shows_its_empty_states_line_above_its_short_action", () => {
  const line =
    "No commits yet in orders, worker, so there's nothing to map. Make a first commit, then check again.";
  const check = { command: "focus", args: [], label: "Check again" };
  const model: PaneModel = {
    ...golden,
    decision: null,
    next: check,
    minimap: { ...golden.minimap, blocks: [], edges: [] },
    empty: { reason: "repos_unborn", line, next: check },
  };
  for (const [columns, rows] of [
    [DOCK.columns, DOCK.rows],
    [80, 8],
  ] as const) {
    const laid = paneRows(model, columns, rows);
    // The whole line, broken over rows that fit, takes the title's place; the press follows it.
    const said = laid.filter((r) => r.key.startsWith("card-empty-"));
    expect(texts(said).join(" ")).toBe(line);
    expect(laid.some((r) => r.key === "card-title")).toBe(false);
    expect(buttons(laid).find((b) => b.key === "next")?.label).toBe(
      "Check again",
    );
    expect(height(laid)).toBeLessThanOrEqual(rows);
    expect(widest(laid)).toBeLessThanOrEqual(columns);
  }
  // A pane with no empty state, or an empty state with no line, keeps its title.
  expect(
    paneRows({ ...golden, decision: null }, DOCK.columns, DOCK.rows).some(
      (r) => r.key === "card-title",
    ),
  ).toBe(true);
  const bare = { ...model, empty: { reason: "repos_unborn", next: check } };
  expect(
    paneRows(bare, DOCK.columns, DOCK.rows).some((r) => r.key === "card-title"),
  ).toBe(true);
  // The line passes the display-sanitizing pass.
  const evil = paneRows(
    {
      ...model,
      empty: {
        reason: "repos_unborn",
        line: "evil‮abc\u001b]52;c;x\u0007",
        next: check,
      },
    },
    DOCK.columns,
    DOCK.rows,
  );
  const drawn = texts(evil).join("\n");
  expect(drawn).toContain("⟨U+202E⟩");
  expect(hasRaw(drawn)).toBe(false);
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
  // scan is a query (DR-81), so the band presses it; share can't be undone, so the band only names it.
  const scan = bandRow(
    {
      model: {
        ...golden,
        next: { command: "scan", args: ["."], label: "Map these 5 repos" },
      },
    },
    80,
  );
  expect(scan.kind).toBe("buttons");
  const share = bandRow(
    {
      model: {
        ...golden,
        next: { command: "share", args: [], label: "Share as an Artifact" },
      },
    },
    80,
  );
  expect(share.kind).toBe("text");
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

/** Whether a control or bidi character other than a line break is left raw in `text`. */
function hasRaw(text: string): boolean {
  return [...text].some((c) => {
    const n = c.codePointAt(0) ?? 0;
    return (
      (n < 0x20 && n !== 0x0a) ||
      (n >= 0x7f && n <= 0x9f) ||
      (n >= 0x202a && n <= 0x202e) ||
      (n >= 0x2066 && n <= 0x2069)
    );
  });
}
