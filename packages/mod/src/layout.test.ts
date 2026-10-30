import {
  CATALOGUE,
  MOD_STRINGS,
  type PaneModel,
  type Picker,
  type PickerEntry,
  type PickerName,
} from "@planroom/contract";
import { expect, test } from "vitest";

import focusReply from "../../../tests/golden/cli/focus.json";
import stageReply from "../../../tests/golden/cli/focus-picker-stage.json";
import {
  M1_PRESENCE,
  M2_DECISION_AND_DIFFS,
  M2_DRIFT,
  overlay,
} from "../plugin-tests/overlays";
import { INITIAL_ENGINE } from "./engine";
import { fallbackFor } from "./fallback";
import {
  bandRow,
  CHOICE_COLUMNS,
  cutHunk,
  DOCK,
  fallbackRows,
  paneRows,
  pickerKeys,
  rowHeight,
  rowWidth,
  type Row,
} from "./layout";
import type { Shown } from "./room";
import type { EngineState } from "./state-types";
import type { View } from "./views";

const golden = (focusReply as unknown as { result: PaneModel }).result;

/** The golden pane's mini-map, which a full-detail `focus` always carries. */
const goldenMap = golden.minimap ?? { level: "system", blocks: [], edges: [] };

/**
 * The engine's pane after a fixture scan has no open decision, diffs, drift or presence yet, so
 * the layout tests also lay the surface tests' milestone overlays over it: a four-choice card with
 * its batch, two diffs, drift with a top item and one agent.
 */
const busy: PaneModel = {
  ...overlay(golden, M2_DECISION_AND_DIFFS, M2_DRIFT, M1_PRESENCE),
  minimap: { ...goldenMap, blocks: goldenMap.blocks.slice(0, 5) },
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
  for (const block of busy.minimap?.blocks ?? []) {
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
  // Every Button has a key, the list's page Button among them (decided: it takes the next free
  // row key).
  const paged: PaneModel = {
    ...busy,
    minimap: { ...goldenMap, blocks: goldenMap.blocks.slice(0, 10) },
  };
  expect(goldenMap.blocks.length).toBeGreaterThanOrEqual(10);
  const rows = paneRows(paged, DOCK.columns, DOCK.rows);
  const keyless = buttons(rows).filter((b) => b.hotkey === undefined);
  expect(keyless.map((b) => b.key)).toEqual([]);
  const hotkeys = buttons(rows).flatMap((b) =>
    b.hotkey === undefined ? [] : [b.hotkey],
  );
  expect(hotkeys.slice(0, 4)).toEqual(["1", "2", "3", "4"]);
  // The two diffs, then the drift line's top item and the who line's top row, then the blocks on
  // the keys left; the page Button takes the next free row key, and the action row's `e` opens the
  // canvas. The selected row is a diff, so `x` and `z` have nothing to do and draw nothing.
  const blockKeys = buttons(rows)
    .filter((b) => b.key.startsWith("block-"))
    .map((b) => b.hotkey);
  expect(blockKeys.length).toBeGreaterThanOrEqual(1);
  expect(hotkeys.slice(4)).toEqual([
    "5",
    "6",
    "7",
    "8",
    ...blockKeys,
    buttons(rows).find((b) => b.key === "page")?.hotkey,
    "l",
    "p",
    "i",
    "e",
  ]);
  expect(blockKeys[0]).toBe("9");
  expect(buttons(rows).find((b) => b.key === "page")?.hotkey).toBe("v");
  expect(buttons(rows).some((b) => b.hotkey === "x" || b.hotkey === "z")).toBe(
    false,
  );
  // Selecting a block draws `x`; a view on the stack draws `z`.
  const down = paneRows(paged, DOCK.columns, DOCK.rows, null, {
    selected: `block-${goldenMap.blocks[0]?.id ?? ""}`,
  });
  const views = [mapView()];
  const up = paneRows(paged, DOCK.columns, DOCK.rows, null, {}, null, views);
  const all = [...buttons(down), ...buttons(up)].flatMap((b) =>
    b.hotkey === undefined ? [] : [b.hotkey],
  );
  expect(all).toContain("x");
  expect(all).toContain("z");
  expect(new Set(hotkeys).size).toBe(hotkeys.length);
});

test("an action key acts on the selected row: the first keyed row until a row key selects another", () => {
  const marked = (view: { selected?: string }) =>
    buttons(paneRows(busy, DOCK.columns, DOCK.rows, null, view))
      .filter((b) => b.label.startsWith("› "))
      .map((b) => b.key);
  expect(marked({})).toEqual(["diff-0"]);
  expect(marked({ selected: "who-0" })).toEqual(["who-0"]);
  // With no diffs, the drift line's top item is first.
  const calm = paneRows({ ...busy, diffs: [] }, DOCK.columns, DOCK.rows);
  expect(
    buttons(calm)
      .filter((b) => b.label.startsWith("› "))
      .map((b) => b.key),
  ).toEqual(["drift-0"]);
  // With nothing above the blocks keyed, the first block is the selected row.
  expect(
    buttons(paneRows(golden, DOCK.columns, DOCK.rows))
      .filter((b) => b.label.startsWith("› "))
      .map((b) => b.key),
  ).toEqual([`block-${goldenMap.blocks[0]?.id ?? ""}`]);
  // With no block drawn either, nothing is marked.
  expect(
    buttons(
      paneRows({ ...golden, minimap: { ...goldenMap, blocks: [] } }, 80, 8),
    ).some((b) => b.label.startsWith("› ")),
  ).toBe(false);
});

test("e opens the canvas as the person's press, and u undoes the last press while it is offered", () => {
  const plain = buttons(paneRows(golden, DOCK.columns, DOCK.rows));
  const canvas = plain.find((b) => b.key === "canvas");
  expect(canvas).toMatchObject({
    hotkey: "e",
    action: { kind: "run", command: "open", args: [] },
  });
  expect(plain.some((b) => b.key === "undo")).toBe(false);
  const undo = {
    command: "edit",
    args: ["--base", "12", "undo", "ev_1"],
    label: "Undo the rename",
  };
  const offered = buttons(
    paneRows(golden, DOCK.columns, DOCK.rows, null, {}, undo),
  );
  expect(offered.find((b) => b.key === "undo")).toMatchObject({
    hotkey: "u",
    action: { kind: "run", command: "edit", undo: true },
  });
  for (const [columns, rows] of [
    [DOCK.columns, DOCK.rows],
    [80, 8],
    [40, 34],
  ] as const) {
    const laid = paneRows(golden, columns, rows, null, {}, undo);
    expect(height(laid)).toBeLessThanOrEqual(rows);
    expect(widest(laid)).toBeLessThanOrEqual(columns);
  }
});

/** A view of the blocks under the pane's first block, as `x` on it shows them. */
function mapView(): View {
  return {
    command: "map",
    args: ["--focus", "orders:repo:orders"],
    seq: golden.focus.seq,
    title: "2 blocks at the container level",
    rows: [
      { id: "orders:container:api", name: "orders/api" },
      { id: "orders:container:db", name: "orders/db" },
    ],
    level: "container",
    line: "2 blocks at the container level",
  };
}

test("a block row takes a row key, and x runs map --focus on the selected block", () => {
  for (const [columns, rows] of [
    [DOCK.columns, DOCK.rows],
    [80, 8],
  ] as const) {
    const laid = paneRows(golden, columns, rows);
    const blocks = buttons(laid).filter((b) => b.key.startsWith("block-"));
    expect(blocks.length, `${columns}x${rows}`).toBeGreaterThanOrEqual(1);
    for (const b of blocks) {
      expect(b.hotkey, b.key).toMatch(/^[0-9a-z]$/);
      expect(b.action).toEqual({ kind: "select", row: b.key });
    }
    // Nothing above them is keyed, so the first block is the selected row and `x` acts on it.
    const id = goldenMap.blocks[0]?.id ?? "";
    expect(blocks[0]?.key).toBe(`block-${id}`);
    expect(buttons(laid).find((b) => b.hotkey === "x")?.action).toMatchObject({
      kind: "run",
      command: "map",
      args: ["--focus", id],
    });
    // The engine opens the level below; the pane names none.
    const downArgs = buttons(laid).find((b) => b.hotkey === "x")?.action;
    expect(downArgs?.kind === "run" ? downArgs.args : []).toEqual([
      "--focus",
      id,
    ]);
    expect(height(laid)).toBeLessThanOrEqual(rows);
    expect(widest(laid)).toBeLessThanOrEqual(columns);
  }
  // A row key selects another block, and `x` follows it.
  const other = goldenMap.blocks[2]?.id ?? "";
  const picked = paneRows(golden, DOCK.columns, DOCK.rows, null, {
    selected: `block-${other}`,
  });
  expect(buttons(picked).find((b) => b.hotkey === "x")?.action).toMatchObject({
    args: ["--focus", other],
  });
  // With a diff selected, `x` has no block to go down into and draws nothing (DR-85).
  expect(
    buttons(paneRows(busy, DOCK.columns, DOCK.rows)).some(
      (b) => b.hotkey === "x",
    ),
  ).toBe(false);
  // `z` draws only while a view is on the stack.
  expect(
    buttons(paneRows(golden, DOCK.columns, DOCK.rows)).some(
      (b) => b.hotkey === "z",
    ),
  ).toBe(false);
  const up = buttons(
    paneRows(golden, DOCK.columns, DOCK.rows, null, {}, null, [mapView()]),
  ).find((b) => b.hotkey === "z");
  expect(up?.action).toEqual({ kind: "up" });
});

test("the top view's title and rows take the place of the blocks, and its mapped rows take keys", () => {
  const laid = paneRows(golden, DOCK.columns, DOCK.rows, null, {}, null, [
    mapView(),
  ]);
  expect(laid.find((r) => r.key === "blocks-title")).toMatchObject({
    text: "2 blocks at the container level",
  });
  // The first row is the selected one, marked.
  expect(texts(laid)).toContain("› □ orders/api");
  expect(texts(laid)).toContain("□ orders/db");
  expect(texts(laid).some((t) => t.endsWith("□ orders"))).toBe(false);
  const keyed = buttons(laid).filter((b) => b.key.startsWith("block-"));
  expect(keyed.map((b) => b.key)).toEqual([
    "block-orders:container:api",
    "block-orders:container:db",
  ]);
  // Its first row is the selected one, so `x` goes down into it.
  expect(buttons(laid).find((b) => b.hotkey === "x")?.action).toMatchObject({
    args: ["--focus", "orders:container:api"],
  });
  // A list view's rows have no ID, so they are plain rows and `x` draws nothing.
  const plain: View = {
    ...mapView(),
    command: "intent lint",
    args: [],
    rows: [{ id: null, name: "Add retries — Names no outcome" }],
    level: null,
  };
  const flat = paneRows(golden, DOCK.columns, DOCK.rows, null, {}, null, [
    plain,
  ]);
  expect(texts(flat)).toContain("□ Add retries — Names no outcome");
  expect(buttons(flat).some((b) => b.key.startsWith("block-"))).toBe(false);
  expect(buttons(flat).some((b) => b.hotkey === "x")).toBe(false);
  expect(buttons(flat).some((b) => b.hotkey === "z")).toBe(true);
});

test("the_who_line_merges_one_actors_rows", () => {
  const you = { kind: "person" as const, id: "you", name: null };
  const row = (
    surface: "mod" | "cli",
    client: string,
    doing: string | null,
  ) => ({
    actor: you,
    surface,
    client_id: client,
    doing,
    focus: null,
    seen_at: "2026-01-01T00:00:00.000000Z",
  });
  const model: PaneModel = {
    ...golden,
    who: [
      row("mod", "mod:s1", "reading orders"),
      row("cli", "cli:41", null),
      row("cli", "cli:42", "reading orders"),
      {
        ...row("mod", "mod:s2", null),
        actor: {
          kind: "agent",
          session: "1a2b3c4d",
          role: "implementer",
          key: null,
        },
      },
    ],
  };
  const who = buttons(paneRows(model, DOCK.columns, DOCK.rows)).find(
    (b) => b.key === "who-0",
  );
  // One name for the actor however many surfaces it is on, with its doings said once.
  expect(who?.label).toContain("you: reading orders");
  expect(who?.label.match(/you/g)).toHaveLength(1);
  expect(who?.label).toContain("implementer 1a2b3c4d");
});

test("the_first_block_is_selected_when_nothing_above_is_keyed", () => {
  const marked = (model: PaneModel, columns: number, rows: number) =>
    buttons(paneRows(model, columns, rows))
      .filter((b) => b.label.startsWith("› "))
      .map((b) => b.key);
  const first = `block-${goldenMap.blocks[0]?.id ?? ""}`;
  expect(marked(golden, DOCK.columns, DOCK.rows)).toEqual([first]);
  expect(marked(golden, 80, 8)).toEqual([first]);
  // A diff, a drift item or a who row above them keeps the selection.
  expect(marked(busy, DOCK.columns, DOCK.rows)).toEqual(["diff-0"]);
  const calm = { ...busy, diffs: [] };
  expect(marked(calm, DOCK.columns, DOCK.rows)).toEqual(["drift-0"]);
  const present = { ...golden, who: busy.who };
  expect(marked(present, DOCK.columns, DOCK.rows)).toEqual(["who-0"]);
  // The compact layout keys no drift or who line, so its first block is selected without a diff.
  expect(marked(calm, 80, 16)).toEqual([first]);
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
      level: "system",
      edges: [],
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
    minimap: { ...goldenMap, blocks: [], edges: [] },
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
    // One build at a time: the building pane offers nothing to press while the build runs.
    expect(buttons(rows).length).toBe(
      status === "unsupported_platform" || status === "building" ? 0 : 1,
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

/** One entry of a synthetic picker: a read of the map, so it may press. */
function entry(n: number, current = false): PickerEntry {
  return {
    label: `Entry ${n}`,
    detail: `${n} items`,
    handle: { command: "map", args: ["--lens", `e${n}`], label: `Show ${n}` },
    current,
  };
}

/** A picker of `count` entries, the first `current` ones in force. */
function pickerOf(
  count: number,
  more = 0,
  name: PickerName = "lens",
  current = 0,
): Picker {
  return {
    name,
    question: "Which one do you want to see?",
    why: "It changes what the map draws.",
    entries: Array.from({ length: count }, (_, i) => entry(i + 1, i < current)),
    more,
    estimate: null,
  };
}

const POOL = [
  "1",
  "2",
  "3",
  "4",
  "5",
  "6",
  "7",
  "8",
  "9",
  "0",
  "b",
  "j",
  "k",
  "q",
  "r",
  "v",
];

/** The picker's own Buttons: the ones with a digit or a pool letter, in row order. */
function keyed(rows: Row[]) {
  return buttons(rows).filter((b) => POOL.includes(b.hotkey ?? ""));
}

test("picker_entries_take_1_to_4_then_the_row_pool", () => {
  const picker = pickerOf(16);
  expect(pickerKeys(picker)).toEqual(POOL);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
  );
  expect(keyed(laid).map((b) => b.hotkey)).toEqual(POOL);
  // Each key runs its own entry's handle as a pick, entry n on key n.
  expect(keyed(laid).map((b) => b.action)).toEqual(
    picker.entries.map((e) => ({
      kind: "run",
      command: "map",
      args: e.handle.args,
      label: e.handle.label,
      pick: "lens",
    })),
  );
  expect(height(laid)).toBeLessThanOrEqual(DOCK.rows);
  expect(widest(laid)).toBeLessThanOrEqual(DOCK.columns);
  // The card's four sit two to a row above the list.
  const pairs = laid.filter(
    (r) => r.kind === "buttons" && r.key.startsWith("picker-card-"),
  );
  expect(
    pairs.map((r) => (r.kind === "buttons" ? r.buttons.length : 0)),
  ).toEqual([2, 2]);
  // The pane's own keys stay out of the digits and the pool while it is open.
  expect(buttons(laid).filter((b) => b.key.startsWith("block-"))).toEqual([]);
  expect(buttons(laid).filter((b) => b.key.startsWith("choice-"))).toEqual([]);
});

test("an_empty_picker_takes_no_key", () => {
  const picker = pickerOf(0);
  expect(pickerKeys(picker)).toEqual([]);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
  );
  expect(keyed(laid)).toEqual([]);
  expect(texts(laid).join("\n")).toContain(MOD_STRINGS.picker_empty);
  expect(texts(laid).join("\n")).toContain(picker.question);
});

test("a_picker_with_only_the_current_entry_takes_no_key", () => {
  const picker = pickerOf(1, 0, "project", 1);
  expect(pickerKeys(picker)).toEqual([null]);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
  );
  expect(keyed(laid)).toEqual([]);
  // It still says what is in force, flat.
  expect(texts(laid).join("\n")).toContain("Entry 1");
  // With another entry to pick, the one in force keeps its place and its key.
  expect(pickerKeys(pickerOf(2, 0, "project", 1))).toEqual(["1", "2"]);
});

test("more_entries_than_keys_show_as_more", () => {
  // Entries the engine left out past the keys' reach are counted on the card.
  const picker = pickerOf(16, 3);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
  );
  expect(texts(laid)).toContain("+3 more");
  expect(texts(laid).filter((t) => /^\+\d+ more$/.test(t))).toHaveLength(1);
  // With little room, the list keeps `room - 1` entries and says how many it cut.
  const tight = paneRows(golden, DOCK.columns, 16, null, {}, null, [], picker);
  expect(height(tight)).toBeLessThanOrEqual(16);
  const kept = keyed(tight).length - 4;
  expect(kept).toBeGreaterThanOrEqual(1);
  expect(kept).toBeLessThan(12);
  expect(texts(tight)).toContain(`+${String(12 - kept)} more`);
  // No page Button turns the picker's list.
  expect(buttons(tight).some((b) => b.action.kind === "page")).toBe(false);
});

test("a_picker_list_keys_entries_5_to_16", () => {
  const picker = pickerOf(16);
  for (const [columns, rows] of [
    [DOCK.columns, DOCK.rows],
    [80, 30],
  ] as const) {
    const laid = paneRows(golden, columns, rows, null, {}, null, [], picker);
    const list = keyed(laid).slice(4);
    expect(list.map((b) => b.hotkey)).toEqual(POOL.slice(4));
    expect(list.map((b) => b.key)).toEqual(
      picker.entries.slice(4).map((_, i) => `picker-entry-${String(i + 5)}`),
    );
    // Each row says its entry: label and detail.
    expect(list[0]?.label).toContain("Entry 5");
    expect(list[0]?.label).toContain("5 items");
    expect(height(laid)).toBeLessThanOrEqual(rows);
    expect(widest(laid)).toBeLessThanOrEqual(columns);
  }
});

test("a_stage_entry_is_a_label_and_lint_keeps_its_key", () => {
  const picker = (stageReply as unknown as { result: Picker }).result;
  // Seven entries: the stages are labels with no key, lint is the seventh entry on the pool's 7.
  expect(pickerKeys(picker)).toEqual([null, null, null, null, null, null, "7"]);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
  );
  const pressable = keyed(laid);
  expect(pressable.map((b) => b.hotkey)).toEqual(["7"]);
  expect(pressable[0]?.action).toMatchObject({
    kind: "run",
    command: "intent lint",
    pick: "stage",
  });
  for (const label of [
    "Intent",
    "Outcome",
    "Breakdown",
    "Next actions",
    "Priorities",
    "Premortem",
  ]) {
    expect(
      texts(laid).some((t) => t.includes(label)),
      label,
    ).toBe(true);
    expect(
      buttons(laid).some((b) => b.label.includes(label)),
      label,
    ).toBe(false);
  }
  // The question carries the cost the engine named.
  expect(
    texts(laid).some((t) => t.startsWith("Draft a stage with Claude?")),
  ).toBe(true);
});

test("a_picker_never_names_a_spending_leaf_as_a_button", () => {
  // A leaf that spends may not be pressed from a picker's key, whatever the engine puts there.
  const spending = {
    name: "intent next",
    idea: "intent",
    summary: "Draft the plan's next stage.",
    actor: "any",
    class: "change",
    budget_tokens: 1500,
    needs_base: false,
    spends: true,
    undo: "edit",
    tool: "room_change",
    room: true,
  } as const;
  const picker: Picker = {
    ...pickerOf(3),
    entries: [
      entry(1),
      {
        ...entry(2),
        handle: { command: "intent next", args: [], label: "Draft" },
      },
      entry(3),
    ],
  };
  const leaves = [...CATALOGUE, spending];
  expect(pickerKeys(picker, leaves)).toEqual(["1", null, "3"]);
  const laid = paneRows(
    golden,
    DOCK.columns,
    DOCK.rows,
    null,
    {},
    null,
    [],
    picker,
    leaves,
  );
  expect(
    buttons(laid).some(
      (b) => b.action.kind === "run" && b.action.command === "intent next",
    ),
  ).toBe(false);
});
