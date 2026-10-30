import {
  CATALOGUE,
  LEAF_OWN_FLAGS,
  MOD_STRINGS,
  TOOL_SPECS,
  type Handle,
  type ToolResult,
} from "@planroom/contract";
import { sanitize } from "@planroom/ui";
import { expect, test } from "vitest";

import focus from "../../../tests/golden/cli/focus.json";
import focusNoRepos from "../../../tests/golden/cli/focus-no-repos.json";
import focusPickerLens from "../../../tests/golden/cli/focus-picker-lens.json";
import focusPickerProject from "../../../tests/golden/cli/focus-picker-project.json";
import focusPickerStage from "../../../tests/golden/cli/focus-picker-stage.json";
import invalidArgument from "../../../tests/golden/cli/invalid-argument.json";
import mapContainer from "../../../tests/golden/cli/map-container.json";
import mapEmpty from "../../../tests/golden/cli/map-empty.json";
import mapGraph from "../../../tests/golden/cli/map-graph.json";
import { bandMayRun, leaf, leavesFor, roomCommands } from "./catalogue";
import { toolResult } from "./engine";
import { handleLine } from "./room";
import { parseRoom, tokenize } from "./room-args";
import {
  changeNeedsNoPrompt,
  hasFlag,
  synopsisLine,
  timeoutFor,
  toolArgv,
  toolSpecs,
} from "./tools";

/** Every golden CLI output a tool can return, by its file's stem; `watch-frames` is a stream. */
const GOLDENS: Record<string, unknown> = {
  focus,
  "focus-no-repos": focusNoRepos,
  "focus-picker-lens": focusPickerLens,
  "focus-picker-project": focusPickerProject,
  "focus-picker-stage": focusPickerStage,
  "invalid-argument": invalidArgument,
  "map-container": mapContainer,
  "map-empty": mapEmpty,
  "map-graph": mapGraph,
};

/** The strip rule every generated line follows: what the handler supplies is never in it. */
const SUPPLIED = [
  "planroom ",
  "[--json]",
  "[--base BASE]",
  "[--idem IDEM]",
  "[--client CLIENT]",
];

/** The tokens a string costs, as the engine estimates them. */
function tokens(text: string): number {
  return Math.ceil(new TextEncoder().encode(text).length / 4);
}

/** A tool result as the handler returns it: one JSON string. */
function serialized(reply: unknown): string {
  return JSON.stringify(
    toolResult({ exitCode: 0, stdout: JSON.stringify(reply), stderr: "" }),
  );
}

test("tool_schemas_equal_catalogue: each tool's specs come from the catalogue, its synopses under the strip rule", () => {
  const specs = toolSpecs();
  expect(specs).toBe(TOOL_SPECS);
  for (const spec of specs) {
    const leaves = CATALOGUE.filter((l) => l.tool === spec.name);
    expect(spec.inputSchema).toMatchObject({
      required: ["command"],
      additionalProperties: false,
      properties: { command: { enum: leaves.map((l) => l.name) } },
    });
    // The intro from en.toml's [tools], then one synopsis line per leaf, in catalogue order.
    const [, ...lines] = spec.description.split("\n");
    expect(lines).toHaveLength(leaves.length);
    leaves.forEach((l, i) => {
      const line = lines[i] ?? "";
      expect(line === l.name || line.startsWith(`${l.name} `), line).toBe(true);
      for (const word of SUPPLIED) expect(line).not.toContain(word);
    });
    expect(spec.isDeferred).toBe(spec.name === "room_handoff");
    expect("payload" in (spec.inputSchema["properties"] as object)).toBe(
      spec.name === "room_change",
    );
  }
  // A tool with no leaf yet isn't registered: an empty enum would only cost tokens.
  const names = specs.map((s) => s.name);
  for (const tool of ["room_query", "room_change", "room_handoff"] as const) {
    expect(names.includes(tool)).toBe(CATALOGUE.some((l) => l.tool === tool));
  }
  // The non-deferred specs are always-on: within the tools' share of the surface.
  const always = specs.filter((s) => !s.isDeferred);
  expect(tokens(JSON.stringify(always))).toBeLessThanOrEqual(
    __PLANROOM_BUDGETS__.surface.tools,
  );
});

test("tool_results_within_budget: every golden as a tool result keeps to budgets.json and its leaf's budget", () => {
  const gated = __PLANROOM_BUDGETS__.tool_results;
  expect(Object.keys(gated).sort()).toEqual(Object.keys(GOLDENS).sort());
  for (const [stem, reply] of Object.entries(GOLDENS)) {
    const result = serialized(reply);
    const command = (reply as { command: string }).command;
    const spent = tokens(result);
    expect(spent, stem).toBeLessThanOrEqual(gated[stem] ?? 0);
    expect(spent, stem).toBeLessThanOrEqual(leaf(command)?.budget_tokens ?? 0);
  }
});

test("a_wrong_argument_returns_the_leaf_synopsis: the mod's refusal and the engine's envelope alike", () => {
  // The engine's: `map --detail nope --json`, as the tool result carries it.
  const engine = JSON.parse(serialized(invalidArgument)) as ToolResult;
  // The mod's own: a flag no tool takes.
  const refused = toolArgv("room_query", {
    command: "map",
    args: ["--client", "x"],
  });
  expect("refused" in refused).toBe(true);
  const mod = "refused" in refused ? refused.refused : null;
  for (const [who, result] of [
    ["engine", engine],
    ["mod", mod],
  ] as const) {
    expect(result?.ok, who).toBe(false);
    const error = result?.error;
    expect(error?.code, who).toBe("invalid_argument");
    const details = error?.details as {
      field?: unknown;
      synopsis?: unknown;
      schema?: Handle;
    };
    expect(typeof details.field, who).toBe("string");
    expect(String(details.synopsis), who).toContain("--level");
    expect(details.schema?.args, who).toEqual(["--command", "map"]);
    expect(error?.hint, who).toBe("planroom schema --command map");
    expect(tokens(JSON.stringify(result)), who).toBeLessThanOrEqual(
      leaf("map")?.budget_tokens ?? 0,
    );
  }
  // The mod's synopsis is the leaf's line in the tool's description, word for word.
  expect((mod?.error?.details as { synopsis: string }).synopsis).toBe(
    synopsisLine("room_query", "map"),
  );
  // A command the tool doesn't serve names the catalogue's handle instead.
  const none = toolArgv("room_query", { command: "scan" });
  expect("refused" in none && none.refused.error?.hint).toBe(
    "planroom schema --commands",
  );
});

/** Every handle in a value, at any depth. */
function handlesIn(value: unknown, out: Handle[] = []): Handle[] {
  if (Array.isArray(value)) {
    for (const v of value) handlesIn(v, out);
  } else if (value !== null && typeof value === "object") {
    const h = value as Partial<Handle>;
    if (
      typeof h.command === "string" &&
      typeof h.label === "string" &&
      Array.isArray(h.args)
    ) {
      out.push(value as Handle);
    }
    for (const v of Object.values(value)) handlesIn(v, out);
  }
  return out;
}

test("handles_round_trip_after_redaction: every golden handle runs through the tools and /room unchanged", () => {
  const handles = Object.values(GOLDENS).flatMap((reply) =>
    handlesIn(JSON.parse(serialized(reply))),
  );
  expect(handles.length).toBeGreaterThan(0);
  // An ID with `=`, quotes and a space survives both ways too.
  handles.push({
    command: "find",
    args: [`ws:svc:a=b "c" 'd' e`],
    label: "Find it",
  });
  for (const h of handles) {
    const l = leaf(h.command);
    expect(l, h.command).toBeDefined();
    for (const arg of h.args) {
      expect(arg).not.toContain("[redacted]");
      expect(sanitize(arg)).toBe(arg);
    }
    if (l?.tool != null) {
      const built = toolArgv(l.tool, { command: h.command, args: h.args });
      expect("argv" in built, h.command).toBe(true);
      if ("argv" in built) {
        const path = h.command.split(" ").length;
        expect(built.argv.slice(path, path + h.args.length)).toEqual(h.args);
      }
    }
    // The line the handle shows runs the same arguments when typed back.
    const line = /`(.*)`$/.exec(handleLine(h))?.[1] ?? "";
    const words = tokenize(line);
    expect(Array.isArray(words)).toBe(true);
    if (!Array.isArray(words)) continue;
    if (words[0] === "/room") {
      const request = parseRoom(line.slice("/room ".length), roomCommands());
      expect(request).toMatchObject({ kind: "run", args: h.args });
    } else {
      expect(words.slice(1 + h.command.split(" ").length)).toEqual(h.args);
    }
  }
});

test("a_stage_proposal_writes_no_file: the payload goes on stdin with exactly one --file", () => {
  const payload = {
    stage: "design",
    items: [{ key: "retries", text: "Add retries" }],
  };
  for (const args of [[], ["--file", "-"], ["--file=-"]]) {
    const built = toolArgv(
      "room_change",
      { command: "intent propose", args, payload },
      { idem: "toolu_7", base: 41 },
    );
    expect("argv" in built).toBe(true);
    if (!("argv" in built)) continue;
    const files = built.argv.filter(
      (a) => a === "--file" || a.startsWith("--file="),
    );
    expect(files, JSON.stringify(args)).toHaveLength(1);
    expect(built.argv[built.argv.indexOf("--base") + 1]).toBe("41");
    expect(built.argv[built.argv.indexOf("--idem") + 1]).toBe("toolu_7");
    expect(built.stdin).toBe(JSON.stringify(payload));
    // No word names a path: the proposal is never a file.
    expect(built.argv.some((a) => a.includes("/"))).toBe(false);
  }
  expect(changeNeedsNoPrompt("intent propose")).toBe(true);
  // A new plan takes no payload and no prompt either.
  const fresh = toolArgv("room_change", {
    command: "intent new",
    args: ["Add retries"],
  });
  expect("argv" in fresh && fresh.stdin).toBeUndefined();
  expect(changeNeedsNoPrompt("intent new")).toBe(true);
});

test("room_query_asks_focus_for_its_summary unless the arguments name a detail", () => {
  expect(toolArgv("room_query", { command: "focus" })).toEqual({
    argv: ["focus", "--detail", "summary", "--json"],
  });
  for (const args of [["--detail", "pane"], ["--detail=pane"]]) {
    expect(toolArgv("room_query", { command: "focus", args })).toEqual({
      argv: ["focus", ...args, "--json"],
    });
  }
});

test("a_tool_refuses_client: the handler names the writer, so --client is no tool argument", () => {
  for (const args of [["--client", "x"], ["--client=x"]]) {
    const out = toolArgv("room_change", { command: "scan", args });
    expect("refused" in out).toBe(true);
    if ("refused" in out) {
      expect(out.refused.error?.message).toBe(MOD_STRINGS.tool_no_client);
      expect(out.refused.error?.details).toMatchObject({ field: "client" });
    }
  }
  expect(hasFlag(["--clients"], "--client")).toBe(false);
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
      "--base",
      "12",
      "--idem",
      "t1",
      ".",
      "--file",
      "-",
      "--json",
    ],
    stdin: '{"items":[]}',
  });
  // The model's own base and idem, in either form, are never doubled.
  expect(
    toolArgv(
      "room_change",
      { command: "edit", args: ["--base=5", "--idem=k", "rename", "a", "b"] },
      { idem: "toolu_1", base: 9 },
    ),
  ).toEqual({
    argv: ["edit", "--base=5", "--idem=k", "rename", "a", "b", "--json"],
  });
});

test("a write's base and idem go right after its path, and only to a leaf whose own flags take them", () => {
  // The supplied flags go before edit's subcommand, which the CLI reads on either side of it.
  expect(
    toolArgv(
      "room_change",
      { command: "edit", args: ["rename", "c:svc:a", "New"] },
      { base: 7, idem: "toolu_1" },
    ),
  ).toEqual({
    argv: [
      "edit",
      "--base",
      "7",
      "--idem",
      "toolu_1",
      "rename",
      "c:svc:a",
      "New",
      "--json",
    ],
  });
  // share is a hand-off with no --base or --idem of its own.
  expect(
    toolArgv(
      "room_handoff",
      { command: "share", args: [], base: 3, idem: "x" },
      { idem: "toolu_2", base: 9 },
    ),
  ).toEqual({ argv: ["share", "--json"] });
  for (const spec of TOOL_SPECS) {
    for (const l of leavesFor(spec.name)) {
      const built = toolArgv(
        spec.name,
        { command: l.name, base: 3 },
        { idem: "k", base: 3 },
      );
      expect("argv" in built, l.name).toBe(true);
      if (!("argv" in built)) continue;
      const own = LEAF_OWN_FLAGS[l.name] ?? [];
      const path = l.name.split(" ").length;
      const after = built.argv.slice(path);
      for (const flag of ["--base", "--idem"]) {
        const at = after.indexOf(flag);
        expect(at >= 0, `${l.name} ${flag}`).toBe(own.includes(flag));
        // Supplied first, so no subcommand or positional comes before it.
        if (at >= 0) expect(at, `${l.name} ${flag}`).toBeLessThan(4);
      }
    }
  }
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
  const scan = toolArgv("room_query", { command: "scan" });
  expect("refused" in scan && scan.refused.error?.message).toBe(
    'room_query has no command "scan".',
  );
  const args = toolArgv("room_query", { command: "map", args: [1] });
  expect("refused" in args && args.refused.error?.message).toBe(
    "args must be an array of strings.",
  );
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

test("the band may press reads, never an irreversible, hand-off, spending or unknown leaf", () => {
  expect(bandMayRun("map")).toBe(true);
  // scan is a query (DR-81): a stray digit only maps the repos again.
  expect(bandMayRun("scan")).toBe(true);
  expect(bandMayRun("share")).toBe(false);
  expect(bandMayRun("handoff merge")).toBe(false);
  for (const l of CATALOGUE.filter((c) => c.spends)) {
    expect(bandMayRun(l.name), l.name).toBe(false);
  }
});
