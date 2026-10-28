import { CATALOGUE } from "@planroom/contract";
import { expect, test } from "vitest";

import { bandMayRun } from "./catalogue";
import { changeNeedsNoPrompt, timeoutFor, toolArgv, toolSpecs } from "./tools";

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
    argv: [
      "scan",
      ".",
      "--base",
      "12",
      "--idem",
      "t1",
      "--file",
      "-",
      "--json",
    ],
    stdin: '{"items":[]}',
  });
});

test("a write's idem defaults to the call's id; a read takes neither base nor idem", () => {
  expect(
    toolArgv("room_change", { command: "scan" }, { idem: "toolu_1", base: 9 }),
  ).toEqual({ argv: ["scan", "--idem", "toolu_1", "--json"] });
  expect(
    toolArgv(
      "room_query",
      { command: "map", base: 3, idem: "x" },
      { idem: "toolu_1", base: 9 },
    ),
  ).toEqual({ argv: ["map", "--json"] });
});

test("edit, intent, claim and curate need no prompt; scan and decide --run do", () => {
  for (const c of ["edit group", "intent propose", "claim", "curate"])
    expect(changeNeedsNoPrompt(c)).toBe(true);
  for (const c of ["scan", "decide", 3, undefined])
    expect(changeNeedsNoPrompt(c)).toBe(false);
});

test("a tool refuses a leaf it doesn't serve and arguments that aren't strings", () => {
  expect(toolArgv("room_query", { command: "scan" })).toEqual({
    refused: 'room_query has no command "scan".',
  });
  expect(toolArgv("room_query", { command: "map", args: [1] })).toEqual({
    refused: "args must be an array of strings.",
  });
});

test("a_tool_call_never_names_another_workspace: --workspace and --workspace= are refused in every tool", () => {
  const tools = ["room_query", "room_change", "room_handoff"] as const;
  for (const tool of tools) {
    for (const leaf of CATALOGUE.filter((l) => l.tool === tool)) {
      for (const args of [
        ["--workspace", "/elsewhere"],
        ["--workspace=/elsewhere"],
        ["--level", "container", "--workspace", "../other"],
      ]) {
        const out = toolArgv(tool, { command: leaf.name, args, base: 3 });
        expect("refused" in out, `${tool} ${leaf.name} ${args.join(" ")}`).toBe(
          true,
        );
      }
      // A word that only starts like the flag is no flag.
      const fine = toolArgv(tool, {
        command: leaf.name,
        args: ["--workspaces"],
      });
      expect("argv" in fine, `${tool} ${leaf.name}`).toBe(true);
    }
  }
});

test("a scan gets process.run's longest wait", () => {
  expect(timeoutFor(["scan", "--json"])).toBe(600_000);
  expect(timeoutFor(["map", "--json"])).toBe(30_000);
});

test("the band may press reads, never an irreversible or unknown leaf", () => {
  expect(bandMayRun("map")).toBe(true);
  // scan is a query (DR-81): a stray digit only maps the repos again.
  expect(bandMayRun("scan")).toBe(true);
  expect(bandMayRun("share")).toBe(false);
  expect(bandMayRun("handoff merge")).toBe(false);
});
