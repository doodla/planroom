/**
 * The mod's wiring and drawing under `claude plugin test`, on all four surfaces. `cargo xtask test`
 * bundles this file into target/mod-test/ with the golden CLI outputs inlined. Each hook a test
 * registers stands for the engine beneath the plugin; `process.run` answers as the resolver would.
 */

import { expect, mock, test } from "claude-code/testing";
import type { On } from "claude-code";

import focusReply from "../../../tests/golden/cli/focus.json";

const ROOT = "/plugins/planroom";
const RESOLVER = `${ROOT}/scripts/planroom`;
const ENGINE = "/repo/target/debug/planroom";
const SURFACES = ["terminal", "desktop", "vscode", "mobile"] as const;
const BAND_SURFACES = ["terminal", "desktop"] as const;
const DOCK = { columns: 49, rows: 34 };

const PANE = {
  plugin: "planroom",
  component: "Pane",
  requestId: "planroom",
  props: {
    title: "Planroom",
    isFocused: false,
    bodyColumns: DOCK.columns,
    placement: "dock",
    scroll: { offset: 0, bodyRows: DOCK.rows },
    view: {},
  },
} as const;

const BAND = {
  plugin: "planroom",
  component: "AbovePrompt",
  requestId: "band",
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 10,
    bodyColumns: 75,
    scroll: { offset: 0, bodyRows: 10 },
    view: {},
  },
} as const;

type Answer = { exitCode: number; stdout: string; stderr?: string };

/** Stand in for the host: the plugin root, the registrations, and the processes the mod runs. */
function host(
  on: On,
  answer: (argv: readonly string[]) => Answer,
  runs: { argv: readonly string[]; env: Record<string, string> }[] = [],
) {
  mock.env(on, { CLAUDE_PLUGIN_ROOT: ROOT });
  on("command.register", (_$, e) => ({ value: { command: e.name } }));
  on("tool.register", (_$, e) => ({
    value: { tool: `mcp__planroom__${e.name}` },
  }));
  on("ui.open", () => ({ value: { isPlaced: true } }));
  on("ui.status", () => ({ value: undefined }));
  on("ui.toast", () => ({ value: undefined }));
  on("session.id", () => ({ value: "5e55i0n1-0000-4000-8000-000000000000" }));
  on("session.start", (_$, e) => ({ cwd: e.cwd }));
  on("process.run", (_$, e) => {
    runs.push({ argv: e.argv, env: e.init?.env ?? {} });
    const a = answer(e.argv);
    return {
      value: {
        exitCode: a.exitCode,
        stdout: a.stdout,
        stderr: a.stderr ?? "",
        isStdoutTruncated: false,
        isStderrTruncated: false,
      },
    };
  });
}

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

/** An engine that resolves and answers focus with the golden PaneModel. */
function readyEngine(argv: readonly string[]): Answer {
  if (argv[0] === RESOLVER && argv[1] === "version") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"version","result":{"version":"0.1.0","catalogue_hash":"ab12"},"meta":{"seq":0}}',
    };
  }
  if (argv[0] === RESOLVER && argv[1] === "status") {
    return {
      exitCode: 0,
      stdout: `{"ok":true,"step":"checkout","path":"${ENGINE}","version":"0.1.0"}`,
    };
  }
  if (argv[0] === ENGINE && argv[1] === "focus") {
    return { exitCode: 0, stdout: JSON.stringify(focusReply) };
  }
  if (argv[0] === ENGINE && argv[1] === "map") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"map","result":{"level":"system","nodes":[]},"meta":{"seq":412,"took_ms":2,"size":{"bytes":9,"tokens_est":3},"truncated":false}}',
    };
  }
  return {
    exitCode: 2,
    stdout: "",
    stderr: `unexpected argv ${argv.join(" ")}`,
  };
}

async function texts(ui: {
  findAll: (q: { type: string }) => Promise<{ text: string }[]>;
}): Promise<string[]> {
  return [
    ...(await ui.findAll({ type: "Text" })),
    ...(await ui.findAll({ type: "Button" })),
  ].map((t) => t.text);
}

test("no_blank_pane: before the engine exists the pane offers the source build, on every surface", async ($, on) => {
  host(on, (argv) =>
    argv[1] === "version"
      ? { exitCode: 7, stdout: NO_RELEASE }
      : { exitCode: 2, stdout: "" },
  );
  await $.session.start({
    cwd: "/work",
    surface: "terminal",
    isInteractive: true,
  });

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
});

test("the pane draws the golden PaneModel on every surface", async ($, on) => {
  host(on, readyEngine);
  await $.session.start({
    cwd: "/work",
    surface: "terminal",
    isInteractive: true,
  });

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    expect((await ui.find({ type: "Text", text: /^▣ shop/ }))?.text).toBe(
      "▣ shop",
    );
    // The fixture's real pane has no open decision, so the next action is the face-up card.
    expect((await ui.find({ key: "next" }))?.text).toContain(
      "Open the containers",
    );
    expect(
      (await ui.find({ key: "block-worker:repo:worker" }))?.text,
    ).toContain("worker");
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

test("room_query runs the engine as the chat agent and returns one JSON string", async ($, on) => {
  const runs: { argv: readonly string[]; env: Record<string, string> }[] = [];
  host(on, readyEngine, runs);
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });

  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--level", "system"],
  });
  expect(typeof called.result).toBe("string");
  expect(JSON.parse(called.result as string)).toEqual({
    ok: true,
    result: { level: "system", nodes: [] },
    meta: { seq: 412 },
  });
  const map = runs.find((r) => r.argv[1] === "map");
  expect(map?.argv).toEqual([ENGINE, "map", "--level", "system", "--json"]);
  expect(map?.env["PLANROOM_ACTOR"]).toBe("agent:5e55i0n1:chat");
});

test("/room help lists the commands and /room alone opens the pane", async ($, on) => {
  host(on, readyEngine);
  await $.session.start({
    cwd: "/work",
    surface: "terminal",
    isInteractive: true,
  });

  const typed = {
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  } as const;
  const help = await $.command.run({ command: "room", args: "help", ...typed });
  expect(help.text).toContain("/room map");
  const opened = await $.command.run({ command: "room", args: "", ...typed });
  expect(opened.text).toBe("Opened the Planroom pane.");
});
