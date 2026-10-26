/**
 * The mod's tools and `/room` under `claude plugin test`: the engine answers from the golden CLI
 * outputs, and each test reads the argv and environment the mod ran it with.
 */

import { expect, test } from "claude-code/testing";

import { ENGINE, host, START } from "./host";

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
  expect(map?.env["PLANROOM_ACTOR"]).toBe("agent:5e55i0n1:chat");
});

test("/room help lists the commands and /room alone opens the pane", async ($, on) => {
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
  expect(opened.text).toBe("Opened the Planroom pane.");
});
