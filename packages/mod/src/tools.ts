/**
 * The mod's tools (architecture.md, "The mod"; contracts.md, "Mod tool schemas"): a flat input of a
 * catalogue `command`, its CLI `args`, `base` and `idem`, and for `room_change` a `payload` passed
 * on stdin. Their specs are generated into `catalogue.ts` as `TOOL_SPECS`, from the catalogue and
 * `en.toml`'s `[tools]`. Pure: register.tsx runs the argv this module builds.
 */

import {
  LEAF_OWN_FLAGS,
  MOD_STRINGS,
  TOOL_SPECS,
  type CatalogueLeaf,
  type Handle,
  type ToolResult,
  type ToolSpecData,
} from "@planroom/contract";
import { fill } from "@planroom/ui";

import { leavesFor, type ToolName } from "./catalogue";

export type { ToolSpecData };

/** The tools that have leaves in this catalogue, each with its generated synopsis lines. */
export function toolSpecs(): readonly ToolSpecData[] {
  return TOOL_SPECS;
}

/** Whether `args` names `flag`, as `--flag` or `--flag=value`. */
export function hasFlag(args: readonly string[], flag: string): boolean {
  return args.some((a) => a === flag || a.startsWith(`${flag}=`));
}

/**
 * Whether a leaf's own flags, read off its clap arguments, include `flag`: `share` takes no
 * `--idem`, and no read takes `--base` or `--idem`.
 */
export function takes(leafName: string, flag: string): boolean {
  return (LEAF_OWN_FLAGS[leafName] ?? []).includes(flag);
}

/**
 * The supplied flags of one write: `--base` and `--idem` with their values, each only when the
 * leaf takes it and `args` doesn't name it already. They go right after the leaf's path, before
 * any subcommand, though the CLI reads a leaf's own flags on either side of it
 * (`edit --base SEQ undo EVENT`).
 */
export function supplied(
  leafName: string,
  args: readonly string[],
  base: number | undefined,
  idem: string | undefined,
): string[] {
  const out: string[] = [];
  if (
    base !== undefined &&
    takes(leafName, "--base") &&
    !hasFlag(args, "--base")
  )
    out.push("--base", String(base));
  if (
    idem !== undefined &&
    idem !== "" &&
    takes(leafName, "--idem") &&
    !hasFlag(args, "--idem")
  )
    out.push("--idem", idem);
  return out;
}

/** A leaf's synopsis line in its tool's description, under the strip rule. */
export function synopsisLine(tool: ToolName, name: string): string | undefined {
  const spec = TOOL_SPECS.find((s) => s.name === tool);
  return spec?.description
    .split("\n")
    .slice(1)
    .find((line) => line === name || line.startsWith(`${name} `));
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
 * A refusal the mod authors, in the registry's `invalid_argument` shape: `field`, the leaf's
 * `synopsis` from its tool's description and its `schema --command LEAF` handle, which the hint
 * names as an exact command, so a wrong argument costs one small read (contracts.md, DR-66).
 */
function refusal(
  tool: ToolName,
  l: CatalogueLeaf | undefined,
  field: string,
  message: string,
): ToolResult {
  const schema: Handle =
    l === undefined
      ? {
          command: "schema",
          args: ["--commands"],
          label: MOD_STRINGS.tool_commands_label,
        }
      : {
          command: "schema",
          args: ["--command", l.name],
          label: fill(MOD_STRINGS.tool_schema_label, { leaf: l.name }),
        };
  const synopsis =
    l === undefined
      ? leavesFor(tool)
          .map((c) => c.name)
          .join(" | ")
      : (synopsisLine(tool, l.name) ?? l.name);
  const words = schema.args.map((a) => (a.includes(" ") ? `"${a}"` : a));
  return {
    ok: false,
    error: {
      code: "invalid_argument",
      category: "usage",
      retry: "never",
      message,
      hint: `planroom schema ${words.join(" ")}`,
      details: { field, synopsis, schema },
    },
    meta: {},
  };
}

/**
 * The argv after the binary, and the stdin, for one tool call; or the refusal to return. A leaf
 * whose own flags take `--base` or `--idem` gets the model's, else the defaults: `idem` always,
 * `base` when the leaf needs one, right after its path (`supplied`); any other leaf, a read or
 * `share`, gets neither. A `room_change` payload goes on stdin, with `--file -` unless
 * the arguments name a file already, so a proposal needs no file and no Write call (DR-66).
 * `focus` gives its summary unless the arguments ask for another detail (architecture.md, "Tools").
 */
export function toolArgv(
  tool: ToolName,
  input: ToolInput,
  defaults: ToolDefaults = {},
): { argv: string[]; stdin?: string } | { refused: ToolResult } {
  const leaf = leavesFor(tool).find((l) => l.name === input.command);
  if (leaf === undefined) {
    return {
      refused: refusal(
        tool,
        undefined,
        "command",
        fill(MOD_STRINGS.tool_no_command, {
          tool,
          command: JSON.stringify(input.command) ?? "undefined",
        }),
      ),
    };
  }
  const given = input.args ?? [];
  if (!Array.isArray(given) || !given.every((a) => typeof a === "string")) {
    return {
      refused: refusal(tool, leaf, "args", MOD_STRINGS.tool_args_not_strings),
    };
  }
  const args: string[] = given;
  // room_query and most of room_change run with no prompt, so no tool may name another
  // workspace: a tool reads and writes only the session's own (DR-52).
  if (hasFlag(args, "--workspace")) {
    return {
      refused: refusal(tool, leaf, "workspace", MOD_STRINGS.tool_no_workspace),
    };
  }
  // The handler's PLANROOM_CLIENT names this session as the writer instance (DR-73).
  if (hasFlag(args, "--client")) {
    return {
      refused: refusal(tool, leaf, "client", MOD_STRINGS.tool_no_client),
    };
  }
  const base =
    typeof input.base === "number"
      ? input.base
      : leaf.needs_base
        ? defaults.base
        : undefined;
  const idem = typeof input.idem === "string" ? input.idem : defaults.idem;
  const argv = [
    ...leaf.name.split(" "),
    ...supplied(leaf.name, args, base, idem),
    ...args,
  ];
  if (leaf.name === "focus" && !hasFlag(args, "--detail")) {
    argv.push("--detail", "summary");
  }
  if (tool === "room_change" && input.payload !== undefined) {
    if (!hasFlag(args, "--file")) argv.push("--file", "-");
    argv.push("--json");
    return { argv, stdin: JSON.stringify(input.payload) };
  }
  argv.push("--json");
  return { argv };
}

/**
 * The environment a tool call runs with, beside the engine's own: the chat agent as the actor,
 * this session as the writer instance and the turn as the cause, never a person token or
 * `PLANROOM_RAW` (DR-64).
 */
export function agentEnv(
  sessionId: string,
  turnId: string | null,
): Record<string, string> {
  const env: Record<string, string> = {
    PLANROOM_ACTOR: `agent:${sessionId.slice(0, 8)}:chat`,
    PLANROOM_CLIENT: `mod:${sessionId}`,
  };
  if (turnId !== null) env["PLANROOM_CAUSE"] = `turn:${turnId}`;
  return env;
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

/** How long a command may run: a first scan of a large repo takes minutes (DR-47). */
export function timeoutFor(argv: readonly string[]): number {
  return argv[0] === "scan" ? 600_000 : 30_000;
}
