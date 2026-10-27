/**
 * The Planroom mod's hooks module: `cargo xtask gen` bundles it into plugin/hooks/planroom-mod.js.
 * It wires `/room`, the tools, the pane and the band to the engine, which it reaches only through
 * the resolver's CLI (DR-6). The logic it calls is pure and tested with vitest; the wiring is
 * tested by `claude plugin test` through the target/mod-test harness.
 */

import { read, update } from "claude-code";
import type {
  EngineInterface,
  RenderInput,
  RenderElement,
  Register,
} from "claude-code";
import { sanitize } from "@planroom/ui";

import { engineAtom, paneAtom, turnSeqAtom } from "./atoms";
import { leaf, roomLeaves } from "./catalogue";
import {
  engineFromVersion,
  engineMissing,
  INITIAL_ENGINE,
  isWindowsRoot,
  parseReply,
  pathFromStatus,
  toolResult,
  unsupportedPlatform,
  type ToolResult,
} from "./engine";
import { fallbackFor } from "./fallback";
import {
  gitFileDir,
  guardCall,
  guardPlaces,
  namedPaths,
  normalize,
  pinnedDataRoot,
  RESOLVE_LIMIT,
  type Denial,
  type GuardPlaces,
} from "./guard";
import {
  bandRow,
  fallbackRows,
  paneRows,
  type Action,
  type Row,
} from "./layout";
import {
  binary,
  clientEnv,
  engineEnv,
  pinDataRoot,
  refreshPane,
  runProcess,
  watchStop,
} from "./live";
import { personEnv, personKey, registerKey } from "./person";
import {
  lineFor,
  replySummary,
  roomRows,
  roomText,
  shownFrom,
  type Ran,
  type RoomRow,
  type Shown,
} from "./room";
import { completions, helpText, parseRoom } from "./room-args";
import type { EngineState } from "./state-types";
import {
  changeNeedsNoPrompt,
  timeoutFor,
  toolArgv,
  toolSpecs,
  type ToolInput,
} from "./tools";
import type { ToolName } from "./catalogue";
import { hasStore } from "./watch";

const PANE = "planroom";
const TITLE = "Planroom";

type $ = EngineInterface;

/** The session's working directory, which names the workspace a key is registered in. */
let workspace = "";

/**
 * What the pane's next action showed when the person last pressed it (ux-1). The pane draws it in
 * place of the press until focus moves on; a reload forgets it, and the press comes back.
 */
let shown: Shown | null = null;

/**
 * Each `/room` run's full result, by the text it returned, so its `CommandOutput` row can draw it
 * whole; the newest few only, and a reload forgets them, so the row then draws its text.
 */
const drawn = new Map<string, { command: string; ran: Ran }>();
const DRAWN = 16;

async function setEngine($: $, state: EngineState): Promise<void> {
  await update($, engineAtom, () => state);
  $.ui.status(
    state.status === "ready" || state.line === null
      ? undefined
      : sanitize(state.line),
  );
}

/**
 * The workspaces whose store this module life has seen hold the person key. A new life checks
 * again, since a store may lose the key, as when `.planroom/` is deleted and rescanned
 * (security-r3-5); a press the store refuses as no person's drops its workspace from here.
 */
let checked = new Set<string>();

/**
 * Once focus answers from a store, register the person key with it, once per workspace in this
 * module's life; a workspace that knows another key turns the pane to its `person_key_mismatch`
 * fallback.
 */
async function registerOnce($: $, rekey = false): Promise<void> {
  if (!rekey && checked.has(workspace)) return;
  const engine = await read($, engineAtom);
  const pane = await read($, paneAtom);
  const bin = binary(engine);
  if (bin === null || pane.model === null || !hasStore(pane.model)) {
    return;
  }
  const outcome = await registerKey($, bin, workspace, rekey);
  if (outcome.kind === "registered") checked.add(workspace);
  if (outcome.kind === "mismatch") {
    await setEngine($, {
      ...engine,
      status: "person_key_mismatch",
      line: outcome.error.message,
    });
  } else if (
    outcome.kind === "registered" &&
    engine.status === "person_key_mismatch"
  ) {
    await setEngine($, { ...engine, status: "ready", line: null });
  }
}

/**
 * Resolve the engine through the resolver, then draw the workspace's focus, which starts the watch
 * loop once the workspace has a store. Idempotent: a second start in one module's life (a retry,
 * or session.start again) replaces the loop, which restarts from the `lastSeq` `$.state` kept.
 */
async function startEngine($: $): Promise<void> {
  watchStop();
  // The person key is made on the first start and kept in $.store across sessions (DR-72).
  await personKey($);
  // The host hands a mod its own directory; it doesn't set CLAUDE_PLUGIN_ROOT for mods (A-14, WS8).
  const root = $.plugin.root;
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
    await registerOnce($);
  }
}

/** The engine's start-up in this module's life; session.start sets it, a reload starts it again. */
let starting: Promise<void> = Promise.resolve();

/** Run one catalogue command line through the engine; the reply as a tool result and its summary. */
async function runCommand(
  $: $,
  argv: string[],
  env: Record<string, string>,
  stdin?: string,
): Promise<Ran> {
  await starting;
  const engine = await read($, engineAtom);
  const bin = binary(engine);
  if (bin === null) {
    return { result: engineMissing(engine), summary: null };
  }
  const run = await runProcess($, [bin, ...argv], env, timeoutFor(argv), stdin);
  return {
    result: toolResult(run),
    summary: replySummary(parseReply(run.stdout)),
  };
}

/**
 * Run a catalogue command as the person: a press, a submit or a `/room` they typed. It carries a
 * one-time token for exactly this line and never `PLANROOM_ACTOR` (DR-64). A write gets a fresh
 * `idem` for this action, and a leaf that needs a base gets the pane's `seq`.
 */
async function personRun(
  $: $,
  command: string,
  args: readonly string[],
): Promise<Ran> {
  const path = command.split(" ");
  const spec = leaf(command);
  const argv = [...path, ...args];
  if (spec !== undefined && spec.class !== "read" && !args.includes("--idem")) {
    argv.push("--idem", crypto.randomUUID());
  }
  if (spec?.needs_base === true && !args.includes("--base")) {
    const pane = await read($, paneAtom);
    if (pane.model !== null) argv.push("--base", String(pane.model.focus.seq));
  }
  argv.push("--json");
  const ran = await runCommand($, argv, await personEnv($, argv, path.length));
  // The store refused the person's own token: it may have lost the key, so check it again.
  if (ran.result.error?.code === "person_required") checked.delete(workspace);
  return ran;
}

/**
 * What a press does: a fallback's resolver step, the key's re-registration, putting the next
 * action back, or a command, whose result the pane shows in place of the press (ux-1).
 */
async function act($: $, action: Action): Promise<void> {
  if (action.kind === "clear") {
    shown = null;
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "fallback" && action.run === "rekey") {
    await registerOnce($, true);
    return;
  }
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
  const seq = (await read($, paneAtom)).model?.focus.seq;
  const ran = await personRun($, action.command, action.args);
  const line = lineFor(action.command, action.label, ran);
  $.ui.toast(ran.result.ok ? `${sanitize(action.label)}: ${line}` : line);
  shown = seq === undefined ? null : shownFrom(action, ran, seq);
  await refreshPane($);
  $.ui.invalidate("ui.render");
  await registerOnce($);
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
              <Text key={row.key} dimColor wrap="truncate-end">
                {row.text}
              </Text>
            ) : row.tone === "warn" ? (
              <Text key={row.key} color="warning" wrap="truncate-end">
                {row.text}
              </Text>
            ) : row.tone === "accent" ? (
              <Text key={row.key} weight="bold" wrap="truncate-end">
                {row.text}
              </Text>
            ) : (
              <Text key={row.key} wrap="truncate-end">
                {row.text}
              </Text>
            );
          case "buttons":
            return (
              <Box key={row.key} flexDirection="row" gap={2}>
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
            return <Code key={row.key} source={row.source} format="diff" />;
        }
      })}
    </Box>
  );
}

/** A `/room` result's rows, drawn at the transcript's width: texts wrap, the JSON is a code block. */
function drawRoom(
  $: $,
  e: RenderInput<"CommandOutput">,
  rows: RoomRow[],
): RenderElement {
  const { Box, Text, Code } = $.ui.resolve(e);
  return (
    <Box flexDirection="column">
      {rows.map((row) =>
        row.kind === "code" ? (
          <Code key={row.key} source={row.source} language="json" />
        ) : row.tone === "bold" ? (
          <Text key={row.key} weight="bold">
            {row.text}
          </Text>
        ) : row.tone === "warn" ? (
          <Text key={row.key} color="warning">
            {row.text}
          </Text>
        ) : row.tone === "dim" ? (
          <Text key={row.key} dimColor>
            {row.text}
          </Text>
        ) : (
          <Text key={row.key}>{row.text}</Text>
        ),
      )}
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

/**
 * One tool call, as the chat agent: `PLANROOM_ACTOR=agent:<session8>:chat`, never a person token.
 * A write's `idem` defaults to the call's `tool_use_id`, its `base` to the latest `meta.seq`
 * returned to the model this turn, else the pane's. The reply is one JSON string, the error object
 * whole on a failure (DR-66).
 */
async function handleTool(
  $: $,
  tool: ToolName,
  input: ToolInput & { tool_use_id?: unknown },
): Promise<{ result: string }> {
  const turn = await read($, turnSeqAtom);
  const pane = await read($, paneAtom);
  const built = toolArgv(tool, input, {
    idem: typeof input.tool_use_id === "string" ? input.tool_use_id : undefined,
    base: turn.seq ?? pane.model?.focus.seq,
  });
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
  const env: Record<string, string> = {
    ...(await engineEnv()),
    PLANROOM_ACTOR: `agent:${session.slice(0, 8)}:chat`,
    PLANROOM_CLIENT: `mod:${session}`,
  };
  if (turn.turnId !== null) env["PLANROOM_CAUSE"] = `turn:${turn.turnId}`;
  const { result } = await runCommand($, built.argv, env, built.stdin);
  const seq = result.meta["seq"];
  if (typeof seq === "number") {
    await update($, turnSeqAtom, (held) => ({ ...held, seq }));
  }
  if (tool === "room_change" && result.ok) {
    // A first scan makes the store: focus then starts the watch loop and the key registers.
    await refreshPane($);
    await registerOnce($);
  }
  return { result: JSON.stringify(result) };
}

/**
 * Where the guard looks, but for the shell's directory: read once per module from the session's
 * environment and the plugin's own folder, as the host names it and where it lands.
 */
let places: GuardPlaces | null = null;

/** The variables that place Planroom's files, as the session started with them. */
interface PlaceEnv {
  HOME: string | undefined;
  CLAUDE_CONFIG_DIR: string | undefined;
  PLANROOM_DATA: string | undefined;
  XDG_DATA_HOME: string | undefined;
  XDG_CONFIG_HOME: string | undefined;
  ZDOTDIR: string | undefined;
}

let startEnv: PlaceEnv | null = null;

/**
 * The session's environment, read once per start: the guard places what it holds from it, and
 * every run the mod starts names the data root it gives (`pinDataRoot`), so the two agree however
 * the environment changes later.
 */
async function placeEnv($: $): Promise<PlaceEnv> {
  startEnv ??= {
    HOME: await $.env.get("HOME"),
    CLAUDE_CONFIG_DIR: await $.env.get("CLAUDE_CONFIG_DIR"),
    PLANROOM_DATA: await $.env.get("PLANROOM_DATA"),
    XDG_DATA_HOME: await $.env.get("XDG_DATA_HOME"),
    XDG_CONFIG_HOME: await $.env.get("XDG_CONFIG_HOME"),
    ZDOTDIR: await $.env.get("ZDOTDIR"),
  };
  return startEnv;
}

async function guardPlacesFor($: $): Promise<GuardPlaces> {
  // security-r3-1: the Bash tool keeps a `cd` between calls, and the host reports where the shell
  // is now (`$.session.cwd()`, which a shell `cd` moves, unlike `root()`), so every call's relative
  // paths resolve from there, never from where the session started.
  const cwd = await $.session.cwd();
  if (places === null) {
    const plugin = $.plugin.root;
    const real = await $.fs
      .stat(plugin, { resolve: true })
      .then((s) => s.realPath)
      .catch(() => undefined);
    places = guardPlaces(cwd, await placeEnv($), {
      workspace: workspace === "" ? await $.session.root() : workspace,
      // security-r3-3: the mod bundle, its guard and the resolver are never an agent's to write.
      plugin: [
        ...new Set(
          [plugin, real]
            .filter((p): p is string => p !== undefined)
            .map((p) => p.replace(/\/+$/, "")),
        ),
      ],
    });
  }
  // DR-74: a linked worktree's or a submodule's state directory is in the git directory its `.git`
  // file names, so the guard holds that one as the workspace's own.
  const gitDirs = [
    ...new Set([
      ...(await gitDirOf($, places.workspace)),
      ...(await gitDirOf($, cwd)),
    ]),
  ];
  return { ...places, cwd, ...(gitDirs.length > 0 ? { gitDirs } : {}) };
}

/** The git directory each folder's `.git` file names, read once per session; a `.git` folder names none. */
let gitFiles = new Map<string, string[]>();

/** The git directory the `.git` file in `folder` names, read as git reads it, or none. */
async function gitDirOf($: $, folder: string): Promise<string[]> {
  const known = gitFiles.get(folder);
  if (known !== undefined) return known;
  const text = await $.fs
    .read(`${folder.replace(/\/+$/, "")}/.git`)
    .catch(() => undefined);
  const dir = text === undefined ? null : gitFileDir(text, folder);
  const out = dir === null ? [] : [dir];
  gitFiles.set(folder, out);
  return out;
}

/** Where a path lands, symlinks followed: the file's real path, else its folder's plus its name. */
async function landing($: $, path: string): Promise<string[]> {
  const own = await $.fs.stat(path, { resolve: true }).catch(() => undefined);
  if (own?.realPath !== undefined) return [own.realPath];
  const cut = path.lastIndexOf("/");
  const folder = await $.fs
    .stat(cut <= 0 ? "/" : path.slice(0, cut), { resolve: true })
    .catch(() => undefined);
  return folder?.realPath === undefined
    ? []
    : [`${folder.realPath.replace(/\/+$/, "")}/${path.slice(cut + 1)}`];
}

/**
 * The guard's verdict on one call, every path it names resolved first (DR-52), from the directory
 * the shell is in for this call. `readOnly` is the host's word that the tool only reads its input.
 */
async function guardVerdict(
  $: $,
  tool: string,
  input: unknown,
  readOnly: boolean,
): Promise<Denial | null> {
  const at = await guardPlacesFor($);
  const named = namedPaths(tool, input, at);
  // security-r2-4: every path is resolved, or the guard asks; none goes unchecked past a limit.
  if (named.length > RESOLVE_LIMIT) {
    return {
      decision: "ask",
      reason:
        "This call names more paths than the Planroom guard resolves, so it can't check where they all lead.",
    };
  }
  const real = new Map<string, string[]>();
  for (const p of named) {
    real.set(p, await landing($, normalize(p, at.cwd, at.home)));
  }
  return guardCall(tool, input, (p) => real.get(p) ?? [], at, readOnly);
}

// The host reads `register` off the bundle's source: a function declared and exported at the top
// level, its parameter spelled `on`. The bundle step keeps it that way (crates/xtask/src/modgen.rs).
export function register(on: Parameters<Register>[0]): void {
  checked = new Set();
  on("session.start", async ($, e, next) => {
    workspace = e.cwd;
    places = null;
    startEnv = null;
    gitFiles = new Map();
    // First, before any run can start: each names the data root the session starts with, the one
    // the guard holds, and waits for it to be read.
    pinDataRoot(placeEnv($).then(pinnedDataRoot));
    // Immediate, so /room focuses the pane mid-turn, while the model is still working (A-22).
    await $.command.register({
      name: "room",
      description:
        "Planroom: focus the pane, or run a map command (/room help lists them)",
      argumentHint: "[help | map | focus | scan …]",
      immediate: true,
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
        // Bare /room focuses the pane and returns no text, so it costs the model nothing (DR-77).
        await $.ui.open({ id: PANE, title: TITLE, focus: true });
        return {};
      case "help":
        return { text: helpText(roomLeaves()) };
      case "error":
        return { text: sanitize(request.message) };
      case "run": {
        // A /room the person typed acts as the person; one a plugin or the SDK ran doesn't.
        const typed =
          e.origin.kind === "composer" || e.origin.kind === "bridge";
        const ran = typed
          ? await personRun($, request.leaf.name, request.args)
          : await runCommand(
              $,
              [...request.leaf.name.split(" "), ...request.args, "--json"],
              await clientEnv($),
            );
        await refreshPane($);
        await registerOnce($);
        // The model reads the summary line and handles; the row draws the whole result (DR-65).
        const text = roomText(request.leaf.name, ran);
        drawn.delete(text);
        drawn.set(text, { command: request.leaf.name, ran });
        for (const old of drawn.keys()) {
          if (drawn.size <= DRAWN) break;
          drawn.delete(old);
        }
        return { text };
      }
    }
  });

  // The main loop's turn: its id is each tool write's cause, and the base resets with it.
  on("turn.start", async ($, e, next) => {
    if (e.agentId === undefined) {
      await update($, turnSeqAtom, () => ({ turnId: e.turnId, seq: null }));
    }
    return next(e);
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

  // The guard, for every tool call of every agent. Planroom's reads are confined to the workspace,
  // and an agent's edit, intent, claim and curate writes are ghosts or proposals, so those need no
  // prompt (DR-52); everything else the guard lets through goes on to the session's own rules.
  on("tool.check", async ($, e, next) => {
    if (e.tool === "mcp__planroom__room_query") {
      return { decision: "allow" as const };
    }
    if (e.tool === "mcp__planroom__room_change") {
      const command = (e.input as { command?: unknown } | null)?.command;
      return changeNeedsNoPrompt(command)
        ? { decision: "allow" as const }
        : next(e);
    }
    const denial = await guardVerdict(
      $,
      e.tool,
      e.input,
      e.isReadOnly === true,
    );
    return denial ?? next(e);
  }).catch(($, e, next) =>
    next.called
      ? next(e)
      : {
          decision: "ask" as const,
          reason: "The Planroom guard couldn't check this call.",
        },
  );

  on("ui.render", { component: "Pane", requestId: PANE }, async ($, e) => {
    const engine = await read($, engineAtom);
    const pane = await read($, paneAtom);
    const columns = e.props.bodyColumns;
    const rows =
      engine.status === "ready" && pane.model !== null
        ? paneRows(pane.model, columns, e.props.scroll.bodyRows, shown)
        : fallbackRows(fallbackFor(engine, pane.error), columns);
    return draw($, e, rows);
  });

  // A `/room` run's row draws its whole result in place of the text the model reads (DR-65).
  on(
    "ui.render",
    { component: "CommandOutput", props: { command: "room" } },
    ($, e, next) => {
      const held = drawn.get(e.props.text);
      return held === undefined
        ? next(e)
        : drawRoom($, e, roomRows(held.command, held.ran));
    },
  );

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
    return draw($, e, [bandRow(source, e.props.bodyColumns, shown)]);
  });
}
