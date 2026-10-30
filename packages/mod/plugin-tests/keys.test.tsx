/**
 * The pane's and the band's keys under `claude plugin test` (architecture.md, "Pane" and "Band";
 * DR-54, DR-77): the key map, the band's rule for what a stray digit may press, the undo a band
 * press offers, and `e`. The engine answers from the golden CLI outputs, with the milestone
 * overlays laid over the golden pane where the engine can't fill a section yet.
 */

import { expect, test } from "claude-code/testing";
import { CATALOGUE, MOD_STRINGS, type CatalogueLeaf } from "@planroom/contract";
import type { PaneModel } from "@planroom/contract";
import { fill, KEY_MAP } from "@planroom/ui";

import focusReply from "../../../tests/golden/cli/focus.json";
import projectPicker from "../../../tests/golden/cli/focus-picker-project.json";
import { bandRow } from "../src/layout";
import {
  BAND,
  BAND_SURFACES,
  ENGINE,
  host,
  PANE,
  readyEngine,
  SESSION,
  START,
  SURFACES,
  type Answer,
} from "./host";
import {
  M1_EDIT_NEXT,
  M1_PRESENCE,
  M2_DECISION_AND_DIFFS,
  M2_DRIFT,
  overlay,
} from "./overlays";

const golden = (focusReply as unknown as { result: PaneModel }).result;

test("key_map_snapshot: 36 keys, each once, each one digit or one lowercase letter", () => {
  expect(KEY_MAP.map((k) => k.key)).toEqual([
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
    "z",
    "x",
    "l",
    "p",
    "a",
    "o",
    "i",
    "h",
    "g",
    "d",
    "y",
    "n",
    "f",
    "w",
    "m",
    "s",
    "u",
    "e",
    "c",
    "t",
  ]);
  const keys = KEY_MAP.map((k) => k.key);
  expect(new Set(keys).size).toBe(36);
  for (const k of keys) expect(k).toMatch(/^[0-9a-z]$/);
});

test("every hotkey the pane and the band draw is in the key map, once a surface", async ($, on) => {
  const reply = {
    ...focusReply,
    result: overlay(golden, M2_DECISION_AND_DIFFS, M2_DRIFT, M1_PRESENCE),
  };
  const h = host(on, (argv) =>
    argv[0] === ENGINE && argv[1] === "focus"
      ? { exitCode: 0, stdout: JSON.stringify(reply) }
      : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();
  const mapped = new Set(KEY_MAP.map((k) => k.key));
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    const hotkeys = (await ui.findAll({ type: "Button" }))
      .map((b) => b.props["hotkey"])
      .filter((k): k is string => typeof k === "string");
    for (const k of hotkeys) expect(mapped.has(k), k).toBe(true);
    expect(new Set(hotkeys).size).toBe(hotkeys.length);
    expect(hotkeys.length).toBeLessThanOrEqual(36);
    // The same holds with the lens picker dealt over the card.
    const lens = (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === "l",
    );
    await ui.press({ key: lens?.key ?? "" });
    await h.clock.settle();
    await ui.redraw();
    const dealt = (await ui.findAll({ type: "Button" }))
      .map((b) => b.props["hotkey"])
      .filter((k): k is string => typeof k === "string");
    expect(dealt).toContain("2");
    for (const k of dealt) expect(mapped.has(k), k).toBe(true);
    expect(new Set(dealt).size).toBe(dealt.length);
    expect(dealt.length).toBeLessThanOrEqual(36);
    await ui.unmount();
  }
});

test("band_never_names_a_spending_leaf: a leaf that can spend is a label, never a Button", async ($, on) => {
  // A spending leaf the catalogue may not hold yet, beside every one it does.
  const synthetic: CatalogueLeaf = {
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
  };
  const leaves = [...CATALOGUE, synthetic];
  for (const l of leaves.filter((c) => c.spends)) {
    const model: PaneModel = {
      ...golden,
      next: { command: l.name, args: [], label: `Run ${l.name}` },
    };
    const row = bandRow({ model }, 80, null, null, leaves);
    expect(row.kind, l.name).toBe("text");
    // Not even as the undo a press offers.
    const offered = bandRow(
      { model: golden },
      80,
      null,
      { command: l.name, args: [], label: "Undo" },
      leaves,
    );
    expect(
      offered.kind === "buttons" &&
        offered.buttons.some(
          (b) => b.action.kind === "run" && b.action.command === l.name,
        ),
    ).toBe(false);
  }
  // Through the host too: every catalogue leaf that spends, as the pane's next action.
  for (const l of CATALOGUE.filter((c) => c.spends)) {
    const reply = {
      ...focusReply,
      result: { ...golden, next: { command: l.name, args: [], label: l.name } },
    };
    const h = host(on, (argv) =>
      argv[0] === ENGINE && argv[1] === "focus"
        ? { exitCode: 0, stdout: JSON.stringify(reply) }
        : readyEngine(argv),
    );
    await $.session.start(START);
    await h.clock.settle();
    for (const surface of BAND_SURFACES) {
      const band = await $.ui.mount({ ...BAND, surface });
      expect(await band.find({ key: "band-primary" })).toBeUndefined();
      await band.unmount();
    }
  }
});

/** The undo an edit's reply names: its first event under compare-and-set. */
const UNDO = {
  command: "edit",
  args: ["--base", "413", "undo", "ev_01JC2R"],
  label: "Undo the rename",
};

/**
 * An engine whose pane offers the rename as its next action, whose edits answer with their undo,
 * and whose pane's `seq` moves on with each write, as a real store's does.
 */
function editingEngine(): (argv: readonly string[]) => Answer {
  let seq = golden.focus.seq;
  return (argv) => {
    if (argv[0] === ENGINE && argv[1] === "focus") {
      const model = overlay(golden, M1_EDIT_NEXT);
      return {
        exitCode: 0,
        stdout: JSON.stringify({
          ...focusReply,
          result: { ...model, focus: { ...model.focus, seq } },
        }),
      };
    }
    if (argv[0] === ENGINE && argv[1] === "edit") {
      seq += 1;
      const undo = argv.includes("undo");
      return {
        exitCode: 0,
        stdout: JSON.stringify({
          schema_version: 1,
          ok: true,
          command: "edit",
          summary: undo ? "Undid the rename." : "Renamed worker to Workers.",
          result: {
            event: `ev_${seq}`,
            count: 1,
            status: "applied",
            targets: ["worker:repo:worker"],
            more_targets: 0,
            // An undo's reply names its own undo too; the band must not offer it.
            undo: undo
              ? { ...UNDO, args: ["--base", String(seq), "undo", `ev_${seq}`] }
              : UNDO,
          },
          meta: { seq },
        }),
      };
    }
    return readyEngine(argv);
  };
}

test("a_band_press_from_the_prompt_offers_undo: for 10 s the band's primary takes it back", async ($, on) => {
  const h = host(on, editingEngine());
  await $.session.start(START);
  await h.clock.settle();

  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  const next = await band.find({ key: "band-primary" });
  expect(next?.props["hotkey"]).toBe("1");
  expect(next?.text).toContain(M1_EDIT_NEXT.patch.next?.label ?? "");
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  await band.redraw();
  // The toast says what the digit did and how to take it back.
  expect(h.toasts.at(-1)).toBe(
    fill(MOD_STRINGS.toast_undo, {
      label: M1_EDIT_NEXT.patch.next?.label ?? "",
      line: "Renamed worker to Workers.",
    }),
  );
  const offered = await band.find({ key: "band-primary" });
  expect(offered?.props["hotkey"]).toBe("1");
  expect(offered?.text).toContain(UNDO.label);

  // Pressed, the undo runs as the person, under compare-and-set, and offers no undo of its own.
  const before = h.runs.length;
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  await band.redraw();
  const undo = h.runs.slice(before).find((r) => r.argv[1] === "edit");
  // The press's own `--idem` goes before the subcommand, beside the handle's `--base`, where the
  // CLI reads edit's flags: the whole line, as the engine parses it.
  expect(undo?.argv).toEqual([
    ENGINE,
    "edit",
    "--idem",
    undo?.argv[3],
    "--base",
    "413",
    "undo",
    "ev_01JC2R",
    "--json",
  ]);
  expect(undo?.argv[3]).toMatch(/^[0-9a-f-]{36}$/);
  expect(h.toasts.at(-1)).not.toContain("unexpected argument");
  expect(undo?.env["PLANROOM_PERSON_TOKEN"]).toBeDefined();
  expect(undo?.env["PLANROOM_ACTOR"]).toBeUndefined();
  const back = await band.find({ key: "band-primary" });
  expect(back?.text).toContain(M1_EDIT_NEXT.patch.next?.label ?? "");
  expect(back?.text).not.toContain(UNDO.label);
  await band.unmount();
});

test("the band's undo goes after 10 s, and the pane's u keeps it until it is used", async ($, on) => {
  const h = host(on, editingEngine());
  await $.session.start(START);
  await h.clock.settle();

  const band = await $.ui.mount({ ...BAND, surface: "desktop" });
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  await band.redraw();
  expect((await band.find({ key: "band-primary" }))?.text).toContain(
    UNDO.label,
  );
  await h.clock.advance(10_000);
  await h.clock.settle();
  await band.redraw();
  expect((await band.find({ key: "band-primary" }))?.text).toContain(
    M1_EDIT_NEXT.patch.next?.label ?? "",
  );
  await band.unmount();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const u = await ui.find({ key: "undo" });
  expect(u?.props["hotkey"]).toBe("u");
  await ui.press({ key: "undo" });
  await h.clock.settle();
  await ui.redraw();
  expect(
    h.runs.some((r) => r.argv[1] === "edit" && r.argv.includes("ev_01JC2R")),
  ).toBe(true);
  expect(await ui.find({ key: "undo" })).toBeUndefined();
  await ui.unmount();
});

test("e_opens_the_canvas_with_a_person_token", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[0] === ENGINE && argv[1] === "open"
      ? {
          exitCode: 0,
          stdout:
            '{"schema_version":1,"ok":true,"command":"open","summary":"Opened the canvas.","result":{"opened":true},"meta":{"seq":412}}',
        }
      : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    const canvas = await ui.find({ key: "canvas" });
    expect(canvas?.props["hotkey"]).toBe("e");
    expect(canvas?.text).toContain(MOD_STRINGS.canvas_label);
    await ui.unmount();
  }
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  await ui.press({ key: "canvas" });
  await h.clock.settle();
  const open = h.runs.find((r) => r.argv[1] === "open");
  expect(open?.argv).toEqual([ENGINE, "open", "--json"]);
  expect(open?.env["PLANROOM_PERSON_TOKEN"]).toBeDefined();
  expect(open?.env["PLANROOM_ACTOR"]).toBeUndefined();
  expect(h.toasts.at(-1)).toContain("Opened the canvas.");
  await ui.unmount();
});

/** The engine's answer to `map --focus ID`: the two blocks under it. */
const UNDER_WORKER = {
  schema_version: 1,
  ok: true,
  command: "map",
  summary: "2 blocks at the container level",
  result: {
    level: "container",
    lens: "runtime",
    focus: "worker:repo:worker",
    nodes: [
      { id: "worker:container:consumer", name: "consumer" },
      { id: "worker:container:scheduler", name: "scheduler" },
    ],
    edges: [],
    hidden: 0,
    empty: null,
  },
  meta: { seq: golden.focus.seq },
};

test("a_block_is_its_row_key_then_x_and_z_comes_back", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[0] === ENGINE && argv[1] === "map" && argv.includes("--focus")
      ? { exitCode: 0, stdout: JSON.stringify(UNDER_WORKER) }
      : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  // The golden pane has no diff, drift or who row, so its blocks take the row keys from the
  // first: the third block is `7`.
  const hotkey = async (key: string) =>
    (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === key,
    );
  const third = await hotkey("7");
  expect(third?.key).toBe("block-worker:repo:worker");
  expect(await hotkey("x")).toBeDefined();
  expect(await hotkey("z")).toBeUndefined();

  // The row key selects the block and runs nothing.
  const before = h.runs.length;
  await ui.press({ key: third?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  expect(h.runs.slice(before).filter((r) => r.argv[1] === "map")).toEqual([]);
  expect(
    (await ui.find({ key: "block-worker:repo:worker" }))?.text.startsWith("› "),
  ).toBe(true);

  // `x` goes down into it through `map --focus`, as the person, naming no level.
  await ui.press({ key: "down" });
  await h.clock.settle();
  await ui.redraw();
  const down = h.runs.slice(before).find((r) => r.argv[1] === "map");
  expect(down?.argv).toEqual([
    ENGINE,
    "map",
    "--focus",
    "worker:repo:worker",
    "--json",
  ]);
  expect(down?.env["PLANROOM_ACTOR"]).toBeUndefined();
  // The view's rows show in place of the pane's blocks, and `z` comes back.
  expect(
    await ui.find({ key: "block-worker:container:consumer" }),
  ).toBeDefined();
  expect(await ui.find({ key: "block-orders:repo:orders" })).toBeUndefined();
  expect((await hotkey("z"))?.key).toBe("up");

  await ui.press({ key: "up" });
  await h.clock.settle();
  await ui.redraw();
  expect(
    await ui.find({ key: "block-worker:container:consumer" }),
  ).toBeUndefined();
  expect(await ui.find({ key: "block-orders:repo:orders" })).toBeDefined();
  expect(await hotkey("z")).toBeUndefined();
  await ui.unmount();
});

/** A map reply for `--lens`, with one block the lens shows. */
const LENS_MAP = {
  schema_version: 1,
  ok: true,
  command: "map",
  summary: "1 block at the container level",
  result: {
    level: "container",
    lens: "flow",
    focus: null,
    nodes: [{ id: "worker:container:consumer", name: "consumer" }],
    edges: [],
    hidden: 0,
    empty: null,
  },
  meta: { seq: golden.focus.seq },
};

/** The project picker with a second project, so there is something to pick. */
const TWO_PROJECTS = (() => {
  const reply = projectPicker as unknown as {
    result: { entries: Record<string, unknown>[] };
  };
  const first = reply.result.entries[0] ?? {};
  return {
    ...projectPicker,
    result: {
      ...reply.result,
      entries: [
        first,
        {
          label: "Drop the old queue",
          detail: "3 items · 0 waiting",
          handle: {
            command: "intent list",
            args: ["--project", "plan:drop-the-old-queue:project"],
            label: "Show this project's plan",
          },
          current: false,
        },
      ],
    },
  };
})();

const PLAN_ITEMS = {
  schema_version: 1,
  ok: true,
  command: "intent list",
  summary: "2 items",
  result: {
    items: [{ name: "Remove the producer" }, { name: "Drain the queue" }],
  },
  meta: { seq: golden.focus.seq },
};

/** An engine that deals the pickers above and answers the reads they run. */
function pickingEngine(): (argv: readonly string[]) => Answer {
  return (argv) => {
    if (argv[0] === ENGINE && argv[1] === "focus" && argv[3] === "project") {
      return { exitCode: 0, stdout: JSON.stringify(TWO_PROJECTS) };
    }
    if (argv[0] === ENGINE && argv[1] === "map" && argv.includes("--lens")) {
      return { exitCode: 0, stdout: JSON.stringify(LENS_MAP) };
    }
    if (argv[0] === ENGINE && argv[1] === "intent" && argv[2] === "list") {
      return { exitCode: 0, stdout: JSON.stringify(PLAN_ITEMS) };
    }
    return readyEngine(argv);
  };
}

test("a_lens_and_a_project_are_a_key_and_a_digit", async ($, on) => {
  const h = host(on, pickingEngine());
  await $.session.start(START);
  await h.clock.settle();
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const hotkey = async (key: string) =>
    (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === key,
    );

  // `l` deals the lens picker, asked as the pane's own focus is.
  const lens = await hotkey("l");
  expect(lens).toBeDefined();
  await ui.press({ key: lens?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  const asked = h.runs.find((r) => r.argv.includes("--picker"));
  expect(asked?.argv).toEqual([
    ENGINE,
    "focus",
    "--picker",
    "lens",
    "--json",
    "--session",
    SESSION,
  ]);
  // The second entry is the digit 2; its pick runs its handle.
  const second = await hotkey("2");
  expect(second?.text).toContain("Request and message");
  const before = h.runs.length;
  await ui.press({ key: second?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  const map = h.runs.slice(before).find((r) => r.argv[1] === "map");
  expect(map?.argv).toEqual([ENGINE, "map", "--lens", "flow", "--json"]);
  // Its nodes show as list rows, and the picker is closed.
  expect(
    await ui.find({ key: "block-worker:container:consumer" }),
  ).toBeDefined();
  expect(await hotkey("2")).toBeUndefined();

  // `p` deals the project picker; its second entry runs that project's items.
  await ui.press({ key: (await hotkey("p"))?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  const projects = h.runs.filter(
    (r) => r.argv[1] === "focus" && r.argv[3] === "project",
  );
  expect(projects).toHaveLength(1);
  const pickProject = await hotkey("2");
  const mark = h.runs.length;
  await ui.press({ key: pickProject?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  const items = h.runs.slice(mark).find((r) => r.argv[1] === "intent");
  expect(items?.argv).toEqual([
    ENGINE,
    "intent",
    "list",
    "--project",
    "plan:drop-the-old-queue:project",
    "--json",
  ]);
  expect(
    await ui.find({ type: "Text", text: /Remove the producer/ }),
  ).toBeDefined();

  // The project picked is the one the stage picker is asked about.
  await ui.press({ key: (await hotkey("i"))?.key ?? "" });
  await h.clock.settle();
  const stages = h.runs.filter(
    (r) => r.argv[1] === "focus" && r.argv[3] === "stage",
  );
  expect(stages.at(-1)?.argv).toEqual([
    ENGINE,
    "focus",
    "--picker",
    "stage",
    "--project",
    "plan:drop-the-old-queue:project",
    "--json",
    "--session",
    SESSION,
  ]);
  await ui.unmount();
});

test("a_pick_replaces_the_views_so_z_comes_back_to_the_pane", async ($, on) => {
  const picking = pickingEngine();
  const h = host(on, (argv): Answer =>
    argv[0] === ENGINE && argv[1] === "map" && argv.includes("--focus")
      ? { exitCode: 0, stdout: JSON.stringify(UNDER_WORKER) }
      : picking(argv),
  );
  await $.session.start(START);
  await h.clock.settle();
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const hotkey = async (key: string) =>
    (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === key,
    );
  const press = async (key: string | undefined) => {
    await ui.press({ key: key ?? "" });
    await h.clock.settle();
    await ui.redraw();
  };

  // `x` goes down into the worker: its two blocks are a view.
  await press((await hotkey("7"))?.key);
  await press("down");
  expect(
    await ui.find({ key: "block-worker:container:scheduler" }),
  ).toBeDefined();

  // A lens pick shows its map as the one view, in place of the view `x` went down into.
  await press((await hotkey("l"))?.key);
  await press((await hotkey("2"))?.key);
  expect(
    await ui.find({ key: "block-worker:container:consumer" }),
  ).toBeDefined();
  expect(
    await ui.find({ key: "block-worker:container:scheduler" }),
  ).toBeUndefined();

  // `z` comes back to the pane's own blocks, not to the worker's, and has nowhere further to go.
  expect((await hotkey("z"))?.key).toBe("up");
  await press("up");
  expect(
    await ui.find({ key: "block-worker:container:scheduler" }),
  ).toBeUndefined();
  expect(await ui.find({ key: "block-orders:repo:orders" })).toBeDefined();
  expect(await hotkey("z")).toBeUndefined();
  await ui.unmount();
});

test("a_picker_owns_the_digits_until_it_closes", async ($, on) => {
  const reply = {
    ...focusReply,
    result: overlay(golden, M2_DECISION_AND_DIFFS),
  };
  const h = host(on, (argv) =>
    argv[0] === ENGINE && argv[1] === "focus" && !argv.includes("--picker")
      ? { exitCode: 0, stdout: JSON.stringify(reply) }
      : argv[0] === ENGINE && argv[1] === "open"
        ? {
            exitCode: 0,
            stdout:
              '{"schema_version":1,"ok":true,"command":"open","summary":"Opened the canvas.","result":{"opened":true},"meta":{"seq":412}}',
          }
        : pickingEngine()(argv),
  );
  await $.session.start(START);
  await h.clock.settle();
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const hotkey = async (key: string) =>
    (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === key,
    );
  // The card's first choice is `1` until a picker is dealt.
  expect((await hotkey("1"))?.key).toBe("choice-accept");

  await ui.press({ key: (await hotkey("l"))?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  expect(await ui.find({ key: "choice-accept" })).toBeUndefined();
  const mark = h.runs.length;
  await ui.press({ key: (await hotkey("1"))?.key ?? "" });
  await h.clock.settle();
  const ran = h.runs.slice(mark).map((r) => r.argv[1]);
  expect(ran).toContain("map");
  expect(ran).not.toContain("decide");

  // Deal it again; any other action key closes it, then acts, and the card returns.
  await ui.redraw();
  await ui.press({ key: (await hotkey("l"))?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  expect(await ui.find({ key: "choice-accept" })).toBeUndefined();
  const next = h.runs.length;
  await ui.press({ key: "canvas" });
  await h.clock.settle();
  await ui.redraw();
  expect(h.runs.slice(next).some((r) => r.argv[1] === "open")).toBe(true);
  expect((await hotkey("1"))?.key).toBe("choice-accept");
  await ui.unmount();
});

test("the_stage_picker_draws_stages_as_labels_and_runs_lint", async ($, on) => {
  const h = host(on, (argv) =>
    argv[0] === ENGINE && argv[1] === "intent" && argv[2] === "lint"
      ? {
          exitCode: 0,
          stdout: JSON.stringify({
            schema_version: 1,
            ok: true,
            command: "intent lint",
            summary: "1 finding",
            result: {
              findings: [{ name: "premortem", message: "no risks named" }],
            },
            meta: { seq: golden.focus.seq },
          }),
        }
      : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const buttons = async () => await ui.findAll({ type: "Button" });
  await ui.press({
    key: (await buttons()).find((b) => b.props["hotkey"] === "i")?.key ?? "",
  });
  await h.clock.settle();
  await ui.redraw();
  const open = await buttons();
  // No stage is a Button; the labels draw as text.
  for (const stage of ["Intent", "Outcome", "Breakdown", "Next actions"]) {
    expect(
      open.some((b) => b.text.includes(stage)),
      stage,
    ).toBe(false);
  }
  expect(await ui.find({ type: "Text", text: /Outcome/ })).toBeDefined();
  // Lint keeps the key of its place, and runs.
  const lint = open.find((b) => b.props["hotkey"] === "7");
  expect(lint?.text).toContain("Lint");
  const mark = h.runs.length;
  await ui.press({ key: lint?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();
  const run = h.runs.slice(mark).find((r) => r.argv[1] === "intent");
  expect(run?.argv.slice(0, 3)).toEqual([ENGINE, "intent", "lint"]);
  expect(run?.argv.at(-1)).toBe("--json");
  expect(
    await ui.find({ type: "Text", text: /premortem — no risks named/ }),
  ).toBeDefined();
  await ui.unmount();
});

/**
 * S9's key counts for the flows M1 draws, measured by real presses from the pane with no list or
 * picker open (DR-54, DR-77). The flow names are feature_list.json's `key_counts`. M2 adds the
 * rest: the two typed `/room` lines, `ask` (each question) and `intent` (state the intent), which
 * no press starts, and the flows that wait on a key or a card M1 doesn't draw yet: search (`s`),
 * evidence (`o`), running a stage, the next or a jump to any other (`i`, then its digit, runs that
 * stage's draft), the accepts (`a`, `y`), the version card, diffs, S4's regroup, the hand-off,
 * merge and sync.
 */
const KEY_COUNTS = [
  {
    flow: "S1: see the map at the top level, its default lens and the next action",
    count: 0,
  },
  { flow: "S1: zoom into a block", count: 2 },
  { flow: "S1: back up a level", count: 1 },
  { flow: "S1 and S2: switch to any lens, the last entry included", count: 2 },
  { flow: "S3: see the lint findings", count: 2 },
];

test("main_flows_within_two_keys: each flow M1 draws takes the counted presses from the pane, and the band's next action takes one", async ($, on) => {
  const lint = {
    schema_version: 1,
    ok: true,
    command: "intent lint",
    summary: "1 finding",
    result: { findings: [{ name: "premortem", message: "no risks named" }] },
    meta: { seq: golden.focus.seq },
  };
  const picking = pickingEngine();
  const h = host(on, (argv): Answer => {
    if (argv[0] === ENGINE && argv[1] === "map" && argv.includes("--focus")) {
      return { exitCode: 0, stdout: JSON.stringify(UNDER_WORKER) };
    }
    if (argv[0] === ENGINE && argv[1] === "intent" && argv[2] === "lint") {
      return { exitCode: 0, stdout: JSON.stringify(lint) };
    }
    return picking(argv);
  });
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const hotkey = async (key: string) =>
    (await ui.findAll({ type: "Button" })).find(
      (b) => b.props["hotkey"] === key,
    );
  const pickerOpen = async () =>
    (await ui.findAll({ type: "Button" })).some((b) =>
      /^picker-(entry|card|row)-/.test(b.key ?? ""),
    );
  const measured = new Map<string, number>();
  let presses = 0;
  const press = async (key: string): Promise<void> => {
    const button = await hotkey(key);
    expect(button, `a Button with hotkey ${key}`).toBeDefined();
    await ui.press({ key: button?.key ?? "" });
    await h.clock.settle();
    await ui.redraw();
    presses++;
  };
  /** Starts a flow from the pane (no list, no picker) unless it begins inside the list. */
  const start = async (inList = false): Promise<void> => {
    expect((await hotkey("z")) !== undefined).toBe(inList);
    expect(await pickerOpen()).toBe(false);
    presses = 0;
  };
  const done = (flow: string): void => {
    measured.set(flow, presses);
  };
  const ran = (from: number, command: string): string[][] =>
    h.runs
      .slice(from)
      .filter((r) => r.argv[1] === command)
      .map((r) => r.argv.slice(1));

  // S1: the map, its default lens and the next action are drawn with no press.
  await start();
  expect(await ui.find({ type: "Text", text: /^▣ / })).toBeDefined();
  expect(
    await ui.find({
      type: "Text",
      text: fill(MOD_STRINGS.lens_line, {
        level: golden.focus.level,
        lens: golden.focus.lens,
      }),
    }),
  ).toBeDefined();
  expect((await ui.find({ key: "next" }))?.text).toContain(golden.next.label);
  done(KEY_COUNTS[0]?.flow ?? "");

  // S1: zoom into a block: its row key, then `x`.
  await start();
  let mark = h.runs.length;
  await press("7");
  await press("x");
  expect(ran(mark, "map")).toEqual([
    ["map", "--focus", "worker:repo:worker", "--json"],
  ]);
  done(KEY_COUNTS[1]?.flow ?? "");

  // S1: back up a level: `z`.
  await start(true);
  await press("z");
  expect(await ui.find({ key: "block-orders:repo:orders" })).toBeDefined();
  done(KEY_COUNTS[2]?.flow ?? "");

  // S1 and S2: any lens, the last entry included: `l`, then that entry's digit.
  await start();
  mark = h.runs.length;
  await press("l");
  const entries = (await ui.findAll({ type: "Button" })).filter((b) =>
    /^picker-entry-/.test(b.key ?? ""),
  );
  const last = entries.at(-1);
  expect(last?.text).toContain("All lenses");
  await press(String(last?.props["hotkey"]));
  expect(ran(mark, "map")).toEqual([["map", "--lens", "all", "--json"]]);
  done(KEY_COUNTS[3]?.flow ?? "");
  // Back to the pane, which the next flow starts from; not counted.
  await ui.press({ key: (await hotkey("z"))?.key ?? "" });
  await h.clock.settle();
  await ui.redraw();

  // S3: the lint findings: `i`, then lint's digit.
  await start();
  mark = h.runs.length;
  await press("i");
  await press("7");
  expect(ran(mark, "intent")).toEqual([["intent", "lint", "--json"]]);
  done(KEY_COUNTS[4]?.flow ?? "");

  for (const { flow, count } of KEY_COUNTS) {
    expect(measured.get(flow), flow).toBe(count);
    expect(count, flow).toBeLessThanOrEqual(2);
  }
  await ui.unmount();

  // The band: the next action is one press of its primary, hotkey `1`, from the empty prompt.
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  const primary = (await band.findAll({ type: "Button" })).find(
    (b) => b.props["hotkey"] === "1",
  );
  expect(primary?.key).toBe("band-primary");
  const before = h.runs.length;
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  const next = golden.next;
  expect(ran(before, next.command)[0]).toEqual([
    next.command,
    ...next.args,
    "--json",
  ]);
  await band.unmount();
});
