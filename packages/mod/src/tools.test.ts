import { CATALOGUE } from "@planroom/contract";
import { expect, test } from "vitest";

import { bandMayRun } from "./catalogue";
import { timeoutFor, toolArgv, toolSpecs } from "./tools";

test("tool_schemas_equal_catalogue: each tool's command enum is its catalogue leaves", () => {
  const specs = toolSpecs();
  for (const spec of specs) {
    const want = CATALOGUE.filter((l) => l.tool === spec.name).map(
      (l) => l.name,
    );
    expect(spec.inputSchema).toMatchObject({
      required: ["command"],
      additionalProperties: false,
      properties: { command: { enum: want } },
    });
    for (const name of want) expect(spec.description).toContain(`${name}: `);
  }
  // A tool with no leaf yet isn't registered: an empty enum would only cost tokens.
  const names = specs.map((s) => s.name);
  for (const tool of ["room_query", "room_change", "room_handoff"] as const) {
    expect(names.includes(tool)).toBe(CATALOGUE.some((l) => l.tool === tool));
  }
  expect(specs.find((s) => s.name === "room_query")?.isDeferred).toBe(false);
});

test("a tool call becomes the leaf's CLI line", () => {
  expect(
    toolArgv("room_query", { command: "map", args: ["--level", "container"] }),
  ).toEqual({
    argv: ["map", "--level", "container", "--json"],
  });
  expect(
    toolArgv("room_change", {
      command: "scan",
      args: ["."],
      base: 12,
      idem: "t1",
      payload: { items: [] },
    }),
  ).toEqual({
    argv: ["scan", ".", "--base", "12", "--idem", "t1", "--json"],
    stdin: '{"items":[]}',
  });
});

test("a tool refuses a leaf it doesn't serve and arguments that aren't strings", () => {
  expect(toolArgv("room_query", { command: "scan" })).toEqual({
    refused: 'room_query has no command "scan".',
  });
  expect(toolArgv("room_query", { command: "map", args: [1] })).toEqual({
    refused: "args must be an array of strings.",
  });
});

test("a scan gets process.run's longest wait", () => {
  expect(timeoutFor(["scan", "--json"])).toBe(600_000);
  expect(timeoutFor(["map", "--json"])).toBe(30_000);
});

test("the band may press reads, never an irreversible or unknown leaf", () => {
  expect(bandMayRun("map")).toBe(true);
  expect(bandMayRun("scan")).toBe(false);
  expect(bandMayRun("handoff merge")).toBe(false);
});
