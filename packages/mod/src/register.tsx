/**
 * The Planroom mod's hooks module: `cargo xtask gen` bundles it into plugin/hooks/planroom-mod.js.
 * It wires `/room`, the tools, the pane and the band to the engine, which it reaches only through
 * the resolver's CLI (DR-6). The logic it calls is pure and tested with vitest; the wiring is
 * tested by `claude plugin test` through the target/mod-test harness.
 */

import { atom, read, update } from "claude-code";
import type {
  EngineInterface,
  RenderInput,
  RenderElement,
  Register,
} from "claude-code";

import { roomLeaves } from "./catalogue";
import {
  engineFromVersion,
  engineMissing,
  INITIAL_ENGINE,
  isWindowsRoot,
  parseReply,
  pathFromStatus,
  toolResult,
  unsupportedPlatform,
  type RunResult,
  type ToolResult,
} from "./engine";
import { fallbackFor } from "./fallback";
import {
  bandRow,
  fallbackRows,
  paneRows,
  type Action,
  type Row,
} from "./layout";
import { completions, helpText, parseRoom } from "./room-args";
import type { EngineState, PaneState } from "./state-types";
import { timeoutFor, toolArgv, toolSpecs, type ToolInput } from "./tools";
import type { ToolName } from "./catalogue";
import type { PaneModel } from "@planroom/contract";

const PANE = "planroom";
const TITLE = "Planroom";

const engineAtom = atom(
  { plugin: "planroom", key: "engine" } as const,
  INITIAL_ENGINE,
  { shape: "engine-1" },
);
const INITIAL_PANE: PaneState = { model: null, error: null };
const paneAtom = atom(
  { plugin: "planroom", key: "pane" } as const,
  INITIAL_PANE,
  { shape: "pane-1" },
);

type $ = EngineInterface;

async function runProcess(
  $: $,
  argv: string[],
  env: Record<string, string>,
  timeoutMs: number,
  stdin?: string,
): Promise<RunResult> {
  try {
    const init =
      stdin === undefined ? { env, timeoutMs } : { env, timeoutMs, stdin };
    const { exitCode, stdout, stderr } = await $.process.run(argv, init);
    return { exitCode, stdout, stderr };
  } catch (err) {
    return {
      exitCode: 1,
      stdout: "",
      stderr: err instanceof Error ? err.message : String(err),
    };
  }
}

async function clientEnv($: $): Promise<Record<string, string>> {
  return { PLANROOM_CLIENT: `mod:${await $.session.id()}` };
}

async function setEngine($: $, state: EngineState): Promise<void> {
  await update($, engineAtom, () => state);
  $.ui.status(state.status === "ready" ? undefined : (state.line ?? undefined));
}

/** Resolve the engine through the resolver, then draw the workspace's focus. Idempotent. */
async function startEngine($: $): Promise<void> {
  const root = await $.env.get("CLAUDE_PLUGIN_ROOT");
  if (root === undefined) {
    await setEngine($, {
      ...INITIAL_ENGINE,
      status: "failed",
      line: "CLAUDE_PLUGIN_ROOT is unset, so the mod can't find its resolver.",
    });
    return;
  }
  if (isWindowsRoot(root)) {
    await setEngine($, unsupportedPlatform());
    return;
  }
  const resolver = `${root}/scripts/planroom`;
  await setEngine($, { ...INITIAL_ENGINE, resolver });
  const env = await clientEnv($);
  // A first resolve may download the engine, so it gets process.run's longest wait.
  let state = engineFromVersion(
    resolver,
    await runProcess($, [resolver, "version", "--json"], env, 600_000),
  );
  if (state.status === "ready") {
    state = {
      ...state,
      path: pathFromStatus(
        await runProcess($, [resolver, "status", "--json"], env, 30_000),
      ),
    };
  }
  await setEngine($, state);
  if (state.status === "ready") {
    await refreshPane($);
  }
}

function binary(engine: EngineState): string | null {
  return engine.status === "ready" ? (engine.path ?? engine.resolver) : null;
}

/** Run `focus --json` and keep its PaneModel, or its error, for the pane and the band. */
async function refreshPane($: $): Promise<void> {
  const bin = binary(await read($, engineAtom));
  if (bin === null) {
    return;
  }
  const session = await $.session.id();
  const run = await runProcess(
    $,
    [bin, "focus", "--json", "--session", session],
    await clientEnv($),
    30_000,
  );
  const reply = parseReply(run.stdout);
  const pane: PaneState =
    reply?.ok === true
      ? { model: reply.result as PaneModel, error: null }
      : {
          model: null,
          error:
            reply?.error?.message ??
            (run.stderr.trim().split("\n")[0] ||
              `focus exited ${run.exitCode}`),
        };
  await update($, paneAtom, () => pane);
}

/** The engine's start-up in this module's life; session.start sets it, a reload starts it again. */
let starting: Promise<void> = Promise.resolve();

/** Run one catalogue command line through the engine; the reply as a tool result. */
async function runCommand(
  $: $,
  argv: string[],
  env: Record<string, string>,
  stdin?: string,
): Promise<ToolResult> {
  await starting;
  const engine = await read($, engineAtom);
  const bin = binary(engine);
  if (bin === null) {
    return engineMissing(engine);
  }
  return toolResult(
    await runProcess($, [bin, ...argv], env, timeoutFor(argv), stdin),
  );
}

/** What a press does: a fallback's resolver step, or a catalogue command. */
async function act($: $, action: Action): Promise<void> {
  if (action.kind === "fallback") {
    if (action.run === "build") {
      const engine = await read($, engineAtom);
      if (engine.resolver !== null) {
        await setEngine($, {
          ...engine,
          status: "building",
          line: "Building the Planroom engine from source…",
        });
        const built = await runProcess(
          $,
          [engine.resolver, "build"],
          await clientEnv($),
          600_000,
        );
        if (built.exitCode !== 0) {
          const said = built.stderr.trim().split("\n").at(-1) ?? "";
          await setEngine($, {
            ...engine,
            status: "failed",
            line: `The source build failed: ${said}`,
          });
          return;
        }
      }
    }
    starting = startEngine($);
    await starting;
    return;
  }
  const result = await runCommand(
    $,
    [...action.command.split(" "), ...action.args, "--json"],
    await clientEnv($),
  );
  $.ui.toast(
    result.ok
      ? `${action.label}: done`
      : (result.error?.message ?? `${action.label} failed`),
  );
  await refreshPane($);
}

/** Rows as elements, sized by the layout to the body they draw in. */
function draw(
  $: $,
  e: RenderInput<"Pane" | "AbovePrompt">,
  rows: Row[],
): RenderElement {
  const { Box, Text, Button, Code } = $.ui.resolve(e);
  return (
    <Box flexDirection="column">
      {rows.map((row) => {
        switch (row.kind) {
          case "text":
            return row.tone === "dim" ? (
              <Text dimColor wrap="truncate-end">
                {row.text}
              </Text>
            ) : row.tone === "warn" ? (
              <Text color="warning" wrap="truncate-end">
                {row.text}
              </Text>
            ) : row.tone === "accent" ? (
              <Text weight="bold" wrap="truncate-end">
                {row.text}
              </Text>
            ) : (
              <Text wrap="truncate-end">{row.text}</Text>
            );
          case "buttons":
            return (
              <Box flexDirection="row" gap={2}>
                {row.buttons.map((b) => {
                  const press = (): Promise<void> => act($, b.action);
                  return b.hotkey === undefined ? (
                    <Button key={b.key} label={b.label} plain onPress={press} />
                  ) : b.primary === true ? (
                    <Button
                      key={b.key}
                      label={b.label}
                      hotkey={b.hotkey}
                      plain
                      variant="primary"
                      onPress={press}
                    />
                  ) : (
                    <Button
                      key={b.key}
                      label={b.label}
                      hotkey={b.hotkey}
                      plain
                      onPress={press}
                    />
                  );
                })}
              </Box>
            );
          case "diff":
            return <Code source={row.source} format="diff" />;
        }
      })}
    </Box>
  );
}

const HANDLER_FAILED = JSON.stringify({
  ok: false,
  error: {
    code: "internal",
    retry: "never",
    message: "The Planroom mod's tool handler failed.",
  },
  meta: {},
} satisfies ToolResult);

async function handleTool(
  $: $,
  tool: ToolName,
  input: ToolInput,
): Promise<{ result: string }> {
  const built = toolArgv(tool, input);
  if ("refused" in built) {
    const refused: ToolResult = {
      ok: false,
      error: {
        code: "invalid_argument",
        retry: "never",
        message: built.refused,
      },
      meta: {},
    };
    return { result: JSON.stringify(refused) };
  }
  const session = await $.session.id();
  const env = {
    PLANROOM_ACTOR: `agent:${session.slice(0, 8)}:chat`,
    PLANROOM_CLIENT: `mod:${session}`,
  };
  const result = await runCommand($, built.argv, env, built.stdin);
  if (tool === "room_change" && result.ok) {
    await refreshPane($);
  }
  return { result: JSON.stringify(result) };
}

// The host reads `register` off the bundle's source: a function declared and exported at the top
// level, its parameter spelled `on`. The bundle step keeps it that way (crates/xtask/src/modgen.rs).
export function register(on: Parameters<Register>[0]): void {
  on("session.start", async ($, e, next) => {
    await $.command.register({
      name: "room",
      description:
        "Planroom: focus the pane, or run a map command (/room help lists them)",
      argumentHint: "[help | map | focus | scan …]",
    });
    // Awaited, so the tools are listed from turn one and the list never changes later (DR-20).
    for (const spec of toolSpecs()) {
      await $.tool.register(spec);
    }
    // A cloud session can start with no surface; session.attach opens the pane then (A-11).
    if (e.surface !== null) {
      void $.ui.open({ id: PANE, title: TITLE });
    }
    // The first prompt doesn't wait for the engine; commands and tools do. startEngine reports its
    // own failures through the engine atom, so a rejection here means the module was unloaded.
    starting = startEngine($).catch(() => undefined);
    return next(e);
  });

  on("session.attach", async ($, e, next) => {
    void $.ui.open({ id: PANE, title: TITLE });
    return next(e);
  });

  on("command.run", { command: "room" }, async ($, e) => {
    const request = parseRoom(e.args, roomLeaves());
    switch (request.kind) {
      case "focus":
        await $.ui.open({ id: PANE, title: TITLE, focus: true });
        return { text: "Opened the Planroom pane." };
      case "help":
        return { text: helpText(roomLeaves()) };
      case "error":
        return { text: request.message };
      case "run": {
        const argv = [
          ...request.leaf.name.split(" "),
          ...request.args,
          "--json",
        ];
        const result = await runCommand($, argv, await clientEnv($));
        await refreshPane($);
        return { text: JSON.stringify(result) };
      }
    }
  });

  on("prompt.autocomplete", async ($, e, next) => {
    const mine = completions(e.text, e.cursor, roomLeaves());
    const below = await next(e);
    return mine.length === 0
      ? below
      : { suggestions: [...below.suggestions, ...mine] };
  });

  // A handler that throws still answers with the one tool-result shape, never a bare failure.
  on("tool.call", { tool: "mcp__planroom__room_query" }, ($, e) =>
    handleTool($, "room_query", e as unknown as ToolInput),
  ).catch(() => ({ result: HANDLER_FAILED }));
  on("tool.call", { tool: "mcp__planroom__room_change" }, ($, e) =>
    handleTool($, "room_change", e as unknown as ToolInput),
  ).catch(() => ({ result: HANDLER_FAILED }));
  on("tool.call", { tool: "mcp__planroom__room_handoff" }, ($, e) =>
    handleTool($, "room_handoff", e as unknown as ToolInput),
  ).catch(() => ({ result: HANDLER_FAILED }));

  // Reads are confined to the workspace, so they need no prompt (DR-52).
  on("tool.check", { tool: "mcp__planroom__room_query" }, () => ({
    decision: "allow" as const,
  }));

  on("ui.render", { component: "Pane", requestId: PANE }, async ($, e) => {
    const engine = await read($, engineAtom);
    const pane = await read($, paneAtom);
    const columns = e.props.bodyColumns;
    const rows =
      engine.status === "ready" && pane.model !== null
        ? paneRows(pane.model, columns, e.props.scroll.bodyRows)
        : fallbackRows(fallbackFor(engine, pane.error), columns);
    return draw($, e, rows);
  });

  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    if (e.props.hasSurvey) {
      return next(e);
    }
    const engine = await read($, engineAtom);
    const pane = await read($, paneAtom);
    const source =
      engine.status === "ready" && pane.model !== null
        ? { model: pane.model }
        : { fallback: fallbackFor(engine, pane.error) };
    return draw($, e, [bandRow(source, e.props.bodyColumns)]);
  });
}
