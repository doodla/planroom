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
import { bandRow } from "../src/layout";
import {
  BAND,
  BAND_SURFACES,
  ENGINE,
  host,
  PANE,
  readyEngine,
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
