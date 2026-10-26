/**
 * The mod's tools (architecture.md, "The mod"; contracts.md, "Mod tool schemas"): a flat input of a
 * catalogue `command`, its CLI `args`, `base` and `idem`, and for `room_change` a `payload` passed
 * on stdin. Pure: register.tsx runs the argv this module builds.
 */

import { leavesFor, type ToolName } from "./catalogue";

export interface ToolSpecData {
  name: ToolName;
  description: string;
  inputSchema: Record<string, unknown>;
  isDeferred: boolean;
}

const INTROS: Record<ToolName, string> = {
  room_query:
    "Read Planroom's map of this system. Pass base from the meta.seq you read.",
  room_change:
    "Change Planroom's map: your writes are ghosts a person accepts. Pass base from the meta.seq you read.",
  room_handoff: "Report and collect Planroom hand-offs, or publish the map.",
};

/** The tools that have leaves in this catalogue, each with its generated synopsis. */
export function toolSpecs(): ToolSpecData[] {
  const tools: ToolName[] = ["room_query", "room_change", "room_handoff"];
  return tools.flatMap((name) => {
    const leaves = leavesFor(name);
    if (leaves.length === 0) {
      return [];
    }
    const properties: Record<string, unknown> = {
      command: { enum: leaves.map((l) => l.name) },
      args: {
        type: "array",
        items: { type: "string" },
        description:
          'the command\'s CLI arguments, such as ["--level", "container"]',
      },
      base: { type: "integer" },
      idem: { type: "string" },
    };
    if (name === "room_change") {
      properties["payload"] = {
        type: "object",
        description: "passed on stdin to a command run with --file -",
      };
    }
    const synopsis = leaves.map((l) => `${l.name}: ${l.summary}`).join("\n");
    return [
      {
        name,
        description: `${INTROS[name]}\n${synopsis}`,
        inputSchema: {
          type: "object",
          required: ["command"],
          additionalProperties: false,
          properties,
        },
        isDeferred: name === "room_handoff",
      },
    ];
  });
}

export interface ToolInput {
  command?: unknown;
  args?: unknown;
  base?: unknown;
  idem?: unknown;
  payload?: unknown;
}

/** The argv after the binary, and the stdin, for one tool call; or the reason it is refused. */
export function toolArgv(
  tool: ToolName,
  input: ToolInput,
): { argv: string[]; stdin?: string } | { refused: string } {
  const leaf = leavesFor(tool).find((l) => l.name === input.command);
  if (leaf === undefined) {
    return {
      refused: `${tool} has no command ${JSON.stringify(input.command)}.`,
    };
  }
  const args = input.args ?? [];
  if (!Array.isArray(args) || !args.every((a) => typeof a === "string")) {
    return { refused: "args must be an array of strings." };
  }
  const argv = [...leaf.name.split(" "), ...args];
  if (typeof input.base === "number") argv.push("--base", String(input.base));
  if (typeof input.idem === "string") argv.push("--idem", input.idem);
  argv.push("--json");
  if (tool === "room_change" && input.payload !== undefined) {
    return { argv, stdin: JSON.stringify(input.payload) };
  }
  return { argv };
}

/** How long a command may run: a first scan of a large repo takes minutes (DR-47). */
export function timeoutFor(argv: readonly string[]): number {
  return argv[0] === "scan" ? 600_000 : 30_000;
}
