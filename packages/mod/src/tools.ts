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

/** What a write falls back to when the model passes none (architecture.md, "Base" and "Idempotency"). */
export interface ToolDefaults {
  /** The call's `tool_use_id`, so a retried call replays instead of writing twice [A-8]. */
  idem?: string | undefined;
  /** The latest `meta.seq` returned to the model this turn, else the pane's `seq`. */
  base?: number | undefined;
}

/**
 * The argv after the binary, and the stdin, for one tool call; or the reason it is refused. A
 * read takes no `--base` or `--idem`. A write takes the model's, else the defaults: `idem` always,
 * `base` when the leaf needs one. A `room_change` payload goes on stdin, with `--file -` unless
 * the arguments name a file already, so a proposal needs no file and no Write call (DR-66).
 */
export function toolArgv(
  tool: ToolName,
  input: ToolInput,
  defaults: ToolDefaults = {},
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
  if (leaf.class !== "read") {
    const base =
      typeof input.base === "number"
        ? input.base
        : leaf.needs_base
          ? defaults.base
          : undefined;
    if (base !== undefined && !args.includes("--base"))
      argv.push("--base", String(base));
    const idem = typeof input.idem === "string" ? input.idem : defaults.idem;
    if (idem !== undefined && idem !== "" && !args.includes("--idem"))
      argv.push("--idem", idem);
  }
  if (tool === "room_change" && input.payload !== undefined) {
    if (!args.includes("--file")) argv.push("--file", "-");
    argv.push("--json");
    return { argv, stdin: JSON.stringify(input.payload) };
  }
  argv.push("--json");
  return { argv };
}

/**
 * Whether `tool.check` allows a `room_change` call with no prompt: an agent's `edit`, `intent`,
 * `claim` and `curate` writes are ghosts or proposals a person accepts; `scan` and `decide --run`
 * go through the session's own permission flow.
 */
export function changeNeedsNoPrompt(command: unknown): boolean {
  if (typeof command !== "string") return false;
  const verb = command.split(" ")[0];
  return (
    verb === "edit" ||
    verb === "intent" ||
    verb === "claim" ||
    verb === "curate"
  );
}

/** At most this many characters of a `/room` result go back as text the model reads (DR-65). */
export const ROOM_TEXT_CHARS = 1_200;

/** A `/room` result as the model's text: compact JSON, cut to about 300 tokens. */
export function roomText(result: unknown): string {
  const text = JSON.stringify(result);
  return text.length <= ROOM_TEXT_CHARS
    ? text
    : `${text.slice(0, ROOM_TEXT_CHARS - 1)}…`;
}

/** How long a command may run: a first scan of a large repo takes minutes (DR-47). */
export function timeoutFor(argv: readonly string[]): number {
  return argv[0] === "scan" ? 600_000 : 30_000;
}
