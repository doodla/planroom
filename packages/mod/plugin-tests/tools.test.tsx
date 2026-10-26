/**
 * The mod's tools, `/room` and the guard under `claude plugin test`: the engine answers from the
 * golden CLI outputs, and each test reads the argv, stdin and environment the mod ran it with.
 */

import { expect, test } from "claude-code/testing";

import { ENGINE, host, readyEngine, SESSION, START, type Answer } from "./host";

const AGENT = "agent:5e55i0n1:chat";

const NOT_FOUND = JSON.stringify({
  schema_version: 1,
  ok: false,
  command: "map",
  error: {
    code: "not_found",
    category: "not_found",
    retry: "never",
    message: "No node matches worker2.",
    hint: "planroom find worker2",
    details: { candidates: ["worker:repo:worker"] },
  },
  meta: { seq: 412, took_ms: 1, size: { bytes: 300, tokens_est: 75 } },
});

const SCANNED = JSON.stringify({
  schema_version: 1,
  ok: true,
  command: "scan",
  result: { map: { level: "system", nodes: [] } },
  meta: { seq: 430, took_ms: 900, size: { bytes: 90, tokens_est: 23 } },
});

test("room_query runs the engine as the chat agent and returns one JSON string", async ($, on) => {
  const h = host(on);
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

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
  const map = h.runs.find((r) => r.argv[1] === "map");
  expect(map?.argv).toEqual([ENGINE, "map", "--level", "system", "--json"]);
  expect(map?.env["PLANROOM_ACTOR"]).toBe(AGENT);
  expect(map?.env["PLANROOM_CLIENT"]).toBe(`mod:${SESSION}`);
  expect(map?.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
});

test("error_envelopes_reach_the_model: the whole error object, in the one JSON string", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[1] === "map" ? { exitCode: 3, stdout: NOT_FOUND } : readyEngine(argv),
  );
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--focus", "worker2"],
  });
  expect(JSON.parse(called.result as string)).toEqual({
    ok: false,
    error: {
      code: "not_found",
      category: "not_found",
      retry: "never",
      message: "No node matches worker2.",
      hint: "planroom find worker2",
      details: { candidates: ["worker:repo:worker"] },
    },
    meta: { seq: 412 },
  });
});

test("a_tool_call_carries_the_agent_actor: room_change writes as the agent, payload on stdin", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[1] === "scan" ? { exitCode: 0, stdout: SCANNED } : readyEngine(argv),
  );
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const called = await $.tool.call({
    tool: "mcp__planroom__room_change",
    command: "scan",
    args: ["."],
    payload: { note: "inline" },
  });
  expect(JSON.parse(called.result as string)).toMatchObject({
    ok: true,
    meta: { seq: 430 },
  });
  const scan = h.runs.find((r) => r.argv[1] === "scan");
  expect(scan?.env["PLANROOM_ACTOR"]).toBe(AGENT);
  expect(scan?.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
  // A write's idem is the call's own id, so a retried call replays (A-8).
  const idem = scan?.argv[scan.argv.indexOf("--idem") + 1];
  expect(typeof idem === "string" && idem.length > 0).toBe(true);
  expect(scan?.argv.slice(-3)).toEqual(["--file", "-", "--json"]);
  expect(scan?.stdin).toBe('{"note":"inline"}');
  // The change redraws the pane.
  expect(h.runs.filter((r) => r.argv[1] === "focus").length).toBe(2);
});

test("/room help lists the commands; bare /room focuses the pane and returns no text", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();

  const typed = {
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  } as const;
  const help = await $.command.run({ command: "room", args: "help", ...typed });
  expect(help.text).toContain("/room map");
  const opened = await $.command.run({ command: "room", args: "", ...typed });
  expect(opened.text).toBeUndefined();
  const ran = await $.command.run({
    command: "room",
    args: "map --level system",
    ...typed,
  });
  // The summary line the model reads, never the raw envelope (ux-2).
  expect(ran.text).toBe("No blocks at the system level");
  expect(h.runs.find((r) => r.argv[1] === "map")?.argv).toEqual([
    ENGINE,
    "map",
    "--level",
    "system",
    "--json",
  ]);
  const unknown = await $.command.run({
    command: "room",
    args: "fly",
    ...typed,
  });
  expect(unknown.text).toContain('no command "fly"');
});
