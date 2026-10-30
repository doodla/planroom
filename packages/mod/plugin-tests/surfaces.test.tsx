/**
 * The mod's drawing under `claude plugin test`, on all four surfaces. `cargo xtask test` bundles
 * this file into target/mod-test/ with the golden CLI outputs inlined. Pane sections the engine
 * can't fill yet draw the milestone overlays from `overlays.ts`, laid over the golden `focus.json`.
 */

import { expect, test, type TestBody } from "claude-code/testing";
import {
  FALLBACK_PANES,
  MOD_STRINGS,
  type PaneModel,
} from "@planroom/contract";
import { fill, sanitize, truncate } from "@planroom/ui";

import focusReply from "../../../tests/golden/cli/focus.json";
import noRepos from "../../../tests/golden/cli/focus-no-repos.json";
import mapContainer from "../../../tests/golden/cli/map-container.json";
import {
  BAND,
  BAND_SURFACES,
  DOCK,
  ENGINE,
  focusRuns,
  host,
  isResolver,
  overlaid,
  PANE,
  readyEngine,
  RESOLVER,
  START,
  SURFACES,
  type Answer,
  type Host,
} from "./host";
import {
  M1_PRESENCE,
  M2_DECISION_AND_DIFFS,
  M2_DRIFT,
  type Overlay,
} from "./overlays";

const NO_RELEASE = JSON.stringify({
  schema_version: 1,
  ok: false,
  command: "version",
  error: {
    code: "engine_unavailable",
    category: "unavailable",
    retry: "never",
    message:
      "Planroom 0.2.0-dev is a development version, so it has no engine release.",
    hint: `${RESOLVER} build`,
    details: {
      reason: "no_release",
      fix: `${RESOLVER} build`,
      est_minutes: 4,
      canvas: true,
    },
  },
  meta: { seq: 0, took_ms: 0, size: { bytes: 400, tokens_est: 100 } },
});

const golden = (focusReply as unknown as { result: PaneModel }).result;

async function texts(ui: {
  findAll: (q: { type: string }) => Promise<{ text: string }[]>;
}): Promise<string[]> {
  return [
    ...(await ui.findAll({ type: "Text" })),
    ...(await ui.findAll({ type: "Button" })),
  ].map((t) => t.text);
}

test("no_blank_pane: before the engine exists the pane offers the source build, on every surface", async ($, on) => {
  const h = host(on, (argv) =>
    argv[1] === "version"
      ? { exitCode: 7, stdout: NO_RELEASE }
      : { exitCode: 2, stdout: "" },
  );
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    expect(
      (await ui.find({ type: "Text", text: /Build the engine from source/ }))
        ?.text,
    ).toBe("Build the engine from source, about 4 minutes?");
    expect((await ui.find({ key: "fallback-action" }))?.text).toContain(
      "Build it",
    );
    for (const t of await texts(ui))
      expect(t.length).toBeLessThanOrEqual(DOCK.columns + 3);
    await ui.unmount();
  }
  for (const surface of BAND_SURFACES) {
    const band = await $.ui.mount({ ...BAND, surface });
    expect((await band.find({ key: "band-primary" }))?.text).toContain(
      "Build it",
    );
    await band.unmount();
  }
  // No engine, so no watch.
  expect(h.children).toHaveLength(0);
});

test("a_host_that_fails_the_check_registers_nothing_and_shows_one_line", async ($, on) => {
  const h = host(on, readyEngine, {
    version: { version: "2.1.300", base: "2.1.300" },
  });
  // The host's own composition, which has no sections of its own here.
  on("prompt.compose", () => ({ sections: [] }));
  await $.session.start(START);
  await h.clock.settle();

  expect(h.commands).toEqual([]);
  expect(h.tools).toEqual([]);
  expect(h.opened).toBe(0);
  expect(h.runs).toEqual([]);
  expect(h.children).toEqual([]);
  expect(h.statuses).toEqual([
    fill(MOD_STRINGS.host_too_old, { version: "2.1.300", minimum: "2.1.310" }),
  ]);

  // Nothing draws: the mod's render hooks pass through, so the host has no band or pane to mount.
  for (const mount of [BAND, PANE]) {
    let drawn = true;
    try {
      await $.ui.mount({ ...mount, surface: "terminal" });
    } catch (err) {
      drawn = !String(err).includes("no implementation for ui.render");
    }
    expect(drawn).toBe(false);
  }

  // The system prompt has no Planroom section.
  const facts = {
    model: "claude-opus-4-5",
    promptModel: "claude-opus-4-5",
    surfaces: ["terminal"],
    tools: [],
    outputStyle: null,
    traits: [],
  } as const;
  const ids = (await $.prompt.compose(facts)).sections.map((s) => s.id);
  expect(ids).not.toContain("planroom:map");
  expect(h.opened).toBe(0);
});

/**
 * A registration or a pane that a hook beneath the mod refuses: a refusal, not a missing member, so
 * the mod stays on. Only what was refused is skipped; the guard, the pane, the band and what did
 * register run as usual, and one status line names what was refused: the first refusal's, kept
 * while the engine is ready, as the copy-out line is, never the failed host's. (A host that lacks a member
 * fails as with mods off; `src/register.test.ts` checks that on a `$` without it, which the kit
 * can't stand in for.)
 */
async function refusedYetOn(
  $: Parameters<TestBody>[0],
  on: Parameters<TestBody>[1],
  c: {
    refuse: "command.register" | "ui.open";
    /** What the line names as refused. */
    what: string;
  },
): Promise<void> {
  const reason = "a hook of another plugin said no";
  const changed = fill(MOD_STRINGS.host_changed, { version: "2.1.310" });
  const line = fill(MOD_STRINGS.host_refused, { what: c.what, reason });
  const anyRefused = MOD_STRINGS.host_refused.split("{")[0] ?? "";
  /** The status line is the refused line, and no other refused line was ever shown. */
  const oneLine = (): void => {
    expect(h.statuses).toContain(line);
    expect(h.statuses.at(-1)).toBe(line);
    expect(
      h.statuses.filter((l) => l !== line && l?.startsWith(anyRefused)),
    ).toEqual([]);
    expect(h.statuses).not.toContain(changed);
  };
  const h = host(on, readyEngine, { refuse: { [c.refuse]: reason } });
  // The session's own hooks beneath the mod, so a hook that passes through has an answer.
  on("tool.check", () => ({ decision: "allow" as const }));
  on("command.run", () => ({ text: "beneath" }));
  on("session.attach", (_$, e) => ({ clientId: e.clientId }));
  const attach = { surface: "terminal", clientId: "terminal:default" } as const;
  await $.session.start(START);
  await h.clock.settle();
  await $.session.attach(attach);
  await h.clock.settle();

  // Only what was refused is skipped: the rest registered, and the engine and its loop run.
  expect(h.commands).toEqual(c.refuse === "command.register" ? [] : ["room"]);
  expect(h.tools).toHaveLength(3);
  expect(focusRuns(h).length).toBeGreaterThan(0);
  // The golden focus answers from a store, so the watch loop started.
  expect(h.children.length).toBeGreaterThan(0);
  // Asked at start and at attach; a refused pane stays closed.
  expect(h.asked).toBe(2);
  expect(h.opened).toBe(c.refuse === "ui.open" ? 0 : 2);
  // One line, its own; never the failed host's.
  oneLine();

  // The guard runs: an agent's read of the store is denied.
  const read = await $.tool.check({
    tool: "Read",
    input: { file_path: ".planroom/store.db" },
  });
  expect(read.decision).toBe("deny");
  // The tools work.
  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--level", "system"],
  });
  expect(JSON.parse(called.result as string)).toMatchObject({ ok: true });
  // The pane and the band draw.
  const pane = await $.ui.mount({ ...PANE, surface: "terminal" });
  expect(await pane.find({ type: "Text", text: /^▣ shop/ })).toBeDefined();
  await pane.unmount();
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  expect(await band.find({ key: "band-primary" })).toBeDefined();
  await band.unmount();
  // A refused /room passes through to what lies beneath; one that registered opens the pane.
  const room = await $.command.run({
    command: "room",
    args: "",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  if (c.refuse === "command.register") {
    expect(room).toEqual({ text: "beneath" });
  } else {
    expect(room).toEqual({});
    expect(h.opened).toBe(1);
  }
  await h.clock.settle();
  oneLine();
}

test("a_refused_registration_leaves_the_mod_and_the_guard_on: a refused command.register", async ($, on) => {
  await refusedYetOn($, on, { refuse: "command.register", what: "/room" });
});

test("a_refused_registration_leaves_the_mod_and_the_guard_on: a refused ui.open", async ($, on) => {
  await refusedYetOn($, on, {
    refuse: "ui.open",
    what: MOD_STRINGS.host_refused_pane,
  });
});

test("a_failed_hosts_tool_handlers_pass_a_rejection_through_untouched", async ($, on) => {
  // A host the check fails, so every hook passes through; nothing beneath answers a tool call.
  const h = host(on, readyEngine, {
    version: { version: "2.1.300", base: "2.1.300" },
  });
  await $.session.start(START);
  await h.clock.settle();
  for (const tool of [
    "mcp__planroom__room_query",
    "mcp__planroom__room_change",
    "mcp__planroom__room_handoff",
  ] as const) {
    let answered: unknown = undefined;
    let rejected: unknown = undefined;
    try {
      answered = await $.tool.call({ tool, command: "map", args: [] });
    } catch (err) {
      rejected = err;
    }
    // The rejection beneath reaches the caller; the mod's own handler-failed answer never does.
    expect(answered, tool).toBeUndefined();
    expect(String(rejected), tool).toContain("tool.call");
  }
  expect(h.runs).toEqual([]);
});

test("the mod finds its resolver under $.plugin.root, with no CLAUDE_PLUGIN_ROOT in its environment", async ($, on) => {
  // The host doesn't set CLAUDE_PLUGIN_ROOT for a mod (A-14, WS8); the live session found it unset.
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();
  const resolverRuns = h.runs.filter((r) =>
    r.argv[0]?.endsWith("/scripts/planroom"),
  );
  expect(resolverRuns.length).toBeGreaterThan(0);
  // The kit's own `$` has no plugin noun, so check the path's shape: absolute, under the plugin.
  for (const r of resolverRuns) {
    expect(r.argv[0]).toMatch(/^\/.+\/scripts\/planroom$/);
    expect(r.argv[0]).not.toContain("undefined");
  }
});

test("the pane draws the golden focus PaneModel on every surface", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    expect((await ui.find({ type: "Text", text: /^▣ shop/ }))?.text).toBe(
      "▣ shop",
    );
    // The fixture's real pane has no open decision, so the next action is the face-up card.
    expect((await ui.find({ key: "next" }))?.text).toContain(
      "Open the containers",
    );
    // A block is a row on a row key now; the first block is the selected row.
    expect(
      (await ui.find({ key: "block-worker:repo:worker" }))?.text,
    ).toContain("□ worker");
    expect(await ui.find({ type: "Text", text: /^No drift/ })).toBeDefined();
    for (const t of await texts(ui))
      expect(t.length).toBeLessThanOrEqual(DOCK.columns + 3);
    await ui.unmount();
  }
  for (const surface of BAND_SURFACES) {
    const band = await $.ui.mount({ ...BAND, surface });
    expect((await band.find({ key: "band-primary" }))?.text).toContain(
      "Open the containers",
    );
    await band.unmount();
  }
});

/** A name that tries OSC 52 and a right-to-left override. */
const EVIL = `evil\u001b]52;c;aGVsbG8=\u0007${String.fromCodePoint(0x202e)}abc`;

/**
 * The container level a press of "Open the containers" shows: the engine's own golden
 * `map --level container`, with one hostile node appended.
 */
const CONTAINERS = JSON.stringify({
  ...mapContainer,
  result: {
    ...mapContainer.result,
    nodes: [
      ...mapContainer.result.nodes,
      { id: "w:svc:e", name: EVIL, counts: {} },
    ],
  },
});

/** The engine's summary line for the container level, as the golden carries it. */
const SUMMARY = mapContainer.summary;

/** A map node as `/room`'s row draws it: `□ name · counts`, in room.ts's count format. */
function nodeRow(node: {
  name: string;
  counts: Record<string, unknown>;
}): string {
  const counts = Object.entries(node.counts)
    .filter(([, n]) => typeof n === "number")
    .map(([level, n]) => `${String(n)} ${level}`);
  return `□ ${node.name}${counts.length === 0 ? "" : ` · ${counts.join(", ")}`}`;
}

function containersEngine(argv: readonly string[]): Answer {
  return argv[1] === "map" && argv.includes("container")
    ? { exitCode: 0, stdout: CONTAINERS }
    : readyEngine(argv);
}

test("ux-1: the next action shows its result in the pane, the band stops offering it, and Back puts it back", async ($, on) => {
  const h = host(on, containersEngine);
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  await ui.press({ key: "next" });
  await h.clock.settle();
  await ui.redraw();
  expect(h.runs.some((r) => r.argv.includes("--focus"))).toBe(false);
  const result = truncate(sanitize(SUMMARY), DOCK.columns);
  expect((await ui.find({ type: "Text", text: result }))?.text).toBe(result);
  expect(await ui.find({ key: "next" })).toBeUndefined();
  expect((await ui.find({ type: "Text", text: /^Blocks · / }))?.text).toBe(
    "Blocks · container",
  );
  // The hostile name is on the list's last page: turn to it.
  await ui.press({ key: "page" });
  await ui.redraw();
  expect((await ui.find({ type: "Text", text: /^□ evil/ }))?.text).toContain(
    "\\x1b]52",
  );
  // The repo-derived name shows its escapes, never the raw OSC 52 or RLO (security-5).
  for (const t of await texts(ui)) {
    expect(hasRaw(t), t).toBe(false);
  }
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  expect(await band.find({ key: "band-primary" })).toBeUndefined();
  expect((await band.find({ type: "Text" }))?.text).toContain(
    SUMMARY.slice(0, 30),
  );
  await band.unmount();

  await ui.press({ key: "back" });
  await h.clock.settle();
  await ui.redraw();
  expect((await ui.find({ key: "next" }))?.text).toContain(
    "Open the containers",
  );
  await ui.unmount();
});

test("ux-2: /room map returns its summary line and draws the full result in its CommandOutput row", async ($, on) => {
  const h = host(on, containersEngine);
  await $.session.start(START);
  await h.clock.settle();

  const ran = await $.command.run({
    command: "room",
    args: "map --level container",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  expect(ran.text).toBe(sanitize(SUMMARY));
  const first = mapContainer.result.nodes[0];
  expect(first).toBeDefined();
  for (const surface of SURFACES) {
    const row = await $.ui.mount({
      plugin: "planroom",
      surface,
      component: "CommandOutput",
      props: {
        command: "room",
        args: "map --level container",
        text: ran.text ?? "",
        isErrored: false,
      },
    });
    expect(
      (await row.find({ type: "Text", text: sanitize(SUMMARY) }))?.text,
    ).toBe(sanitize(SUMMARY));
    if (first !== undefined) {
      const drawnRow = sanitize(nodeRow(first));
      const rows = await row.findAll({ type: "Text", text: drawnRow });
      expect(rows.map((r) => r.text)).toContain(drawnRow);
    }
    expect((await row.find({ type: "Text", text: /^□ evil/ }))?.text).toContain(
      "\\x1b]52",
    );
    await row.unmount();
  }
});

test("no_decision_shows_next_on_every_surface", async ($, on) => {
  // The golden pane first, then the same engine with an open card laid over it.
  let overlays: Overlay[] = [];
  const h = host(on, (argv) => overlaid(...overlays)(argv));
  await $.session.start(START);
  await h.clock.settle();
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    const next = await ui.find({ key: "next" });
    expect(next?.text).toContain(golden.next.label);
    expect(next?.props["hotkey"]).toBe("1");
    expect(
      await ui.find({ type: "Button", text: /^Accept both$/ }),
    ).toBeUndefined();
    await ui.unmount();
  }
  for (const surface of BAND_SURFACES) {
    const band = await $.ui.mount({ ...BAND, surface });
    const primary = await band.find({ key: "band-primary" });
    expect(primary?.text).toContain(golden.next.label);
    expect(primary?.props["hotkey"]).toBe("1");
    await band.unmount();
  }

  overlays = [M2_DECISION_AND_DIFFS];
  await $.session.start(START);
  await h.clock.settle();
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    const question = M2_DECISION_AND_DIFFS.patch.decision?.question ?? "";
    expect((await ui.find({ type: "Text", text: question }))?.text).toBe(
      question,
    );
    expect(await ui.find({ key: "next" })).toBeUndefined();
    await ui.unmount();
  }
});

test("drift_and_who_top_rows_carry_row_keys", async ($, on) => {
  const h = host(on, overlaid(M2_DECISION_AND_DIFFS, M2_DRIFT, M1_PRESENCE));
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    const hotkey = async (key: string) =>
      (await ui.find({ key }))?.props["hotkey"];
    expect(await hotkey("diff-0")).toBe("5");
    expect(await hotkey("diff-1")).toBe("6");
    expect(await hotkey("drift-0")).toBe("7");
    expect(await hotkey("who-0")).toBe("8");
    const buttons = await ui.findAll({ type: "Button" });
    const choices = buttons
      .filter((b) => b.key?.startsWith("choice-") === true)
      .map((b) => b.props["hotkey"]);
    expect(choices).toEqual(["1", "2", "3", "4"]);
    const keys = buttons
      .map((b) => b.props["hotkey"])
      .filter((k) => k !== undefined);
    expect(new Set(keys).size).toBe(keys.length);
    // A block row takes the next row key after the diffs, drift and who, as `x` goes down into it.
    const block = buttons.find((b) => b.key?.startsWith("block-") === true);
    expect(block?.props["hotkey"]).toBe("9");
    await ui.unmount();
  }

  // A row key selects its row and runs nothing; the selected row is marked.
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const before = h.runs.length;
  const seven = (await ui.findAll({ type: "Button" })).find(
    (b) => b.props["hotkey"] === "7",
  );
  expect(seven?.key).toBe("drift-0");
  await ui.press({ key: seven?.key ?? "drift-0" });
  await h.clock.settle();
  await ui.redraw();
  expect(h.runs).toHaveLength(before);
  expect((await ui.find({ key: "drift-0" }))?.text.startsWith("› ")).toBe(true);
  expect((await ui.find({ key: "diff-0" }))?.text.startsWith("› ")).toBe(false);
  await ui.unmount();
});

test("compact_keeps_the_top_diff_line", async ($, on) => {
  const h = host(on, overlaid(M2_DECISION_AND_DIFFS));
  await $.session.start(START);
  await h.clock.settle();

  for (const [columns, rows] of [
    [80, 8],
    [100, 10],
  ] as const) {
    const ui = await $.ui.mount({
      ...PANE,
      surface: "terminal",
      props: {
        ...PANE.props,
        bodyColumns: columns,
        placement: "inline",
        scroll: { offset: 0, bodyRows: rows },
      },
    });
    expect(await ui.find({ key: "diff-row-0" })).toBeDefined();
    expect((await ui.find({ key: "diff-0" }))?.props["hotkey"]).toBe("5");
    expect(await ui.find({ key: "diff-row-1" })).toBeUndefined();
    // The drift and who lines move behind their keys; the card stays.
    expect(await ui.find({ type: "Text", text: /^No drift/ })).toBeUndefined();
    expect(await ui.find({ key: "drift-0" })).toBeUndefined();
    expect(await ui.find({ key: "who-0" })).toBeUndefined();
    expect(
      await ui.find({ type: "Text", text: /^Nobody else is here/ }),
    ).toBeUndefined();
    expect(
      await ui.find({
        type: "Text",
        text: M2_DECISION_AND_DIFFS.patch.decision?.question ?? "",
      }),
    ).toBeDefined();
    // Every row fits: the body holds them all, and none is wider than it.
    const drawn = (await ui.drawn()) as { props: { children?: unknown } };
    const children = drawn.props.children;
    expect(Array.isArray(children) ? children.length : 1).toBeLessThanOrEqual(
      rows,
    );
    for (const t of await texts(ui))
      expect(t.length).toBeLessThanOrEqual(columns + 3);
    await ui.unmount();
  }
});

test("pane_shows_the_engines_summary_line", async ($, on) => {
  const h = host(on, containersEngine);
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  await ui.press({ key: "next" });
  await h.clock.settle();
  await ui.redraw();
  // The engine's own line, sanitized and cut to the dock: the mod composes none of it.
  const result = truncate(sanitize(SUMMARY), DOCK.columns);
  expect((await ui.find({ type: "Text", text: result }))?.text).toBe(result);
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  expect((await band.find({ type: "Text" }))?.text).toContain(
    sanitize(SUMMARY).slice(0, 30),
  );
  await band.unmount();
  await ui.unmount();

  const ran = await $.command.run({
    command: "room",
    args: "map --level container",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  expect(ran.text).toBe(sanitize(SUMMARY));
});

test("a_check_again_press_refreshes_the_pane_with_no_way_back", async ($, on) => {
  const h = host(on, (argv) =>
    argv[0] === ENGINE && argv[1] === "focus"
      ? { exitCode: 0, stdout: JSON.stringify(noRepos) }
      : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  const next = await ui.find({ key: "next" });
  expect(next?.text).toBe(noRepos.result.next.label);
  const focused = focusRuns(h).length;
  await ui.press({ key: "next" });
  await h.clock.settle();
  await ui.redraw();
  // One more focus, as the person's token never needs: no toast, and no Back to offer.
  expect(focusRuns(h)).toHaveLength(focused + 1);
  expect(
    h.runs.filter((r) => r.argv[1] === "focus").at(-1)?.env,
  ).not.toHaveProperty("PLANROOM_PERSON_TOKEN");
  expect(h.toasts).toEqual([]);
  expect(await ui.find({ key: "back" })).toBeUndefined();
  expect((await ui.find({ key: "next" }))?.text).toBe(
    noRepos.result.next.label,
  );
  await ui.unmount();
});

test("a_running_source_build_offers_no_second_build", async ($, on) => {
  let built = false;
  const engine = async (argv: readonly string[]): Promise<Answer> => {
    if (isResolver(argv[0]) && argv[1] === "build") {
      await h.clock.sleep(60_000);
      built = true;
      return { exitCode: 0, stdout: "" };
    }
    if (isResolver(argv[0]) && argv[1] === "version" && !built) {
      return { exitCode: 7, stdout: NO_RELEASE };
    }
    return readyEngine(argv);
  };
  const h: Host = host(on, engine);
  await $.session.start(START);
  await h.clock.settle();

  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  // Pressed, not awaited: the build runs on the mocked clock.
  const pressed = ui.press({ key: "fallback-action" });
  await h.clock.advance(1_000);
  await $.session.start(START);
  await h.clock.settle();
  const mid = await $.ui.mount({ ...PANE, surface: "desktop" });
  expect(
    (await mid.find({ type: "Text", text: /^Building the engine/ }))?.text,
  ).toBeDefined();
  expect(await mid.find({ key: "fallback-action" })).toBeUndefined();
  await mid.unmount();
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  expect(await band.find({ key: "band-primary" })).toBeUndefined();
  await band.unmount();
  const builds = () =>
    h.runs.filter((r) => isResolver(r.argv[0]) && r.argv[1] === "build");
  expect(builds()).toHaveLength(1);

  await h.clock.advance(60_000);
  await pressed;
  await h.clock.settle();
  await ui.redraw();
  expect(builds()).toHaveLength(1);
  expect((await ui.find({ key: "next" }))?.text).toContain(golden.next.label);
  await ui.unmount();
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

const ENGINE2 = "/repo/target/release/planroom";

test("a_pruned_binary_resolves_once_then_runs: a press on a gone binary resolves again and runs once on the new one", async ($, on) => {
  let statuses = 0;
  const h = host(on, (argv): Answer => {
    if (isResolver(argv[0]) && argv[1] === "status") {
      // The first resolve names the checkout's build; a later one finds the release build.
      const path = statuses++ === 0 ? ENGINE : ENGINE2;
      return {
        exitCode: 0,
        stdout: `{"ok":true,"step":"checkout","path":"${path}","version":"0.1.0"}`,
      };
    }
    return argv[0] === ENGINE2
      ? readyEngine([ENGINE, ...argv.slice(1)])
      : readyEngine(argv);
  });
  await $.session.start(START);
  await h.clock.settle();
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });

  h.gone.add(ENGINE);
  const mark = h.runs.length;
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  await band.redraw();
  const after = h.runs.slice(mark);
  const maps = after.filter((r) => r.argv[1] === "map");
  expect(maps.map((r) => r.argv[0])).toEqual([ENGINE, ENGINE2]);
  // The same request twice: the same argv after the binary, so the same `--idem`, and the person's
  // one token for the line.
  expect(maps[1]?.argv.slice(1)).toEqual(maps[0]?.argv.slice(1));
  expect(maps[1]?.env).toEqual(maps[0]?.env);
  expect(
    h.runs.filter((r) => isResolver(r.argv[0]) && r.argv[1] === "version"),
  ).toHaveLength(2);
  // The press ran, so the band shows its result and stops offering the next action.
  expect(await band.find({ key: "band-primary" })).toBeUndefined();
  await band.unmount();

  // The new binary goes too, and the resolver still names it: one more resolve, then the line.
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  await ui.press({ key: "back" });
  await h.clock.settle();
  await ui.redraw();
  h.gone.add(ENGINE2);
  await ui.press({ key: "next" });
  await h.clock.settle();
  await ui.redraw();
  // The pane's one line is cut to the dock's width, as every line there is.
  const line = truncate(
    sanitize(fill(MOD_STRINGS.engine_gone, { path: ENGINE2 })),
    DOCK.columns,
  );
  expect((await ui.find({ type: "Text", text: line }))?.text).toBe(line);
  expect(
    (await ui.find({ type: "Text", text: FALLBACK_PANES.failed.headline }))
      ?.text,
  ).toBe(FALLBACK_PANES.failed.headline);
  await ui.unmount();
});

test("a_temporary_host_shows_its_copy_out_line_once: the first temporary_host warning sets the status line, a later one changes nothing, and a line about the engine takes its place", async ($, on) => {
  const warned = (repo: string, ref: string): string =>
    JSON.stringify({
      ...mapContainer,
      meta: {
        ...mapContainer.meta,
        warnings: [
          {
            code: "temporary_host",
            message: "This host loses its files with its container.",
            details: { repo, ref },
          },
        ],
      },
    });
  const lineFor = (repo: string, ref: string): string =>
    sanitize(fill(MOD_STRINGS.temporary_host_copy_out, { repo, ref }));
  let maps = 0;
  const h = host(on, (argv): Answer => {
    if (argv[0] === ENGINE && argv[1] === "map") {
      const first = maps++ === 0;
      return {
        exitCode: 0,
        stdout: first
          ? warned("/work/a", "refs/planroom/backup/a")
          : warned("/work/b", "refs/planroom/backup/b"),
      };
    }
    return readyEngine(argv);
  });
  await $.session.start(START);
  await h.clock.settle();

  // The golden band's primary runs `map`; its reply carries the warning for repo A.
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  await band.press({ key: "band-primary" });
  await h.clock.settle();
  await band.unmount();
  const first = lineFor("/work/a", "refs/planroom/backup/a");
  expect(h.statuses.at(-1)).toBe(first);
  expect(first).toContain("/work/a");
  expect(first).toContain("refs/planroom/backup/a");

  // A second run answers repo B: no status holds B, and the last is still A's.
  const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
  await ui.press({ key: "back" });
  await h.clock.settle();
  await ui.redraw();
  await ui.press({ key: "next" });
  await h.clock.settle();
  await ui.redraw();
  expect(maps).toBe(2);
  expect(h.statuses.some((s) => s?.includes("/work/b"))).toBe(false);
  expect(h.statuses.at(-1)).toBe(first);

  // The engine's own line takes the status over the copy-out line.
  h.gone.add(ENGINE);
  await ui.press({ key: "back" });
  await h.clock.settle();
  await ui.redraw();
  await ui.press({ key: "next" });
  await h.clock.settle();
  expect(h.statuses.at(-1)).toBe(
    sanitize(fill(MOD_STRINGS.engine_gone, { path: ENGINE })),
  );
  await ui.unmount();
});
