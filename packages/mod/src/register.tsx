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
  SessionVersion,
} from "claude-code";
import { MOD_STRINGS, type Handle, type Picker } from "@planroom/contract";
import { fill, sanitize } from "@planroom/ui";

import { engineAtom, outboxAtom, paneAtom, turnSeqAtom } from "./atoms";
import { leaf, leafOfArgv, roomCommands } from "./catalogue";
import {
  afterNoReply,
  copyOutLine,
  engineFromVersion,
  engineGone,
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
  PICKER_OPEN_KEYS,
  type Action,
  type PaneView,
  type Row,
} from "./layout";
import {
  askPicker,
  binary,
  clientEnv,
  engineEnv,
  pinDataRoot,
  refreshPane,
  runProcess,
  watchHalt,
  watchResume,
  watchStop,
} from "./live";
import { add, byPerson, clear, entryFor, resend, stale } from "./outbox";
import { personEnv, personKey, registerKey } from "./person";
import { composeSections } from "./prompt";
import {
  lineFor,
  replySummary,
  ROOM_COMMAND,
  roomPlan,
  roomRows,
  roomText,
  shareContext,
  shownFrom,
  warningsOf,
  type Ran,
  type RoomRow,
  type Shown,
  undoOf,
} from "./room";
import {
  changedLine,
  checkHost,
  isMissing,
  refusedLine,
  showHostLine,
} from "./host";
import { completions } from "./room-args";
import type { EngineState } from "./state-types";
import {
  agentEnv,
  changeNeedsNoPrompt,
  supplied,
  takes,
  timeoutFor,
  toolArgv,
  toolSpecs,
  type ToolInput,
} from "./tools";
import type { ToolName } from "./catalogue";
import { closes } from "./pickers";
import { current, viewFrom, type View } from "./views";
import { hasStore } from "./watch";

const PANE = "planroom";
const TITLE = MOD_STRINGS.title;

type $ = EngineInterface;

/** The session's working directory, which names the workspace a key is registered in. */
let workspace = "";

/**
 * What the pane's next action showed when the person last pressed it (ux-1). The pane draws it in
 * place of the press until focus moves on; a reload forgets it, and the press comes back.
 */
let shown: Shown | null = null;

/**
 * The list's page and the row a row key selected, which an action key then acts on (DR-54). Both
 * belong to one pane: they reset when its `seq` or level moves on, and a reload forgets them.
 */
let view: PaneView = {};
let viewOf = "";

/**
 * The views `x` went down into, the top one drawn in place of the pane's blocks and `z` taking one
 * off; a pick replaces them with its one view. They belong to one pane, as `shown` does: they clear
 * when its `seq` moves, and a reload forgets them.
 */
let views: View[] = [];

/**
 * The picker a key dealt, drawn in place of the card until a pick or another action key closes it,
 * and the last project the person picked, which the stage picker is asked about. A reload forgets
 * both, as it forgets the pressed result.
 */
let picker: Picker | null = null;
let project: string | null = null;

/** The project a pick named, as `--project` in its arguments; null when it named none. */
function projectOf(args: readonly string[]): string | null {
  const at = args.indexOf("--project");
  return at < 0 ? null : (args[at + 1] ?? null);
}

/** The view for the pane at `seq` and `level`, reset when either moved on since the last draw. */
function viewFor(seq: number, level: string): PaneView {
  const at = `${seq}\0${level}`;
  if (at !== viewOf) {
    viewOf = at;
    view = {};
  }
  views = [...current(views, seq)];
  return view;
}

/**
 * Set while the resolver's source build runs: one build at a time in this module's life. A second
 * press of Build it does nothing, and a start-up waits for the build's own, so a `session.start`
 * mid-build can't turn the building pane back into the offer.
 */
let building = false;

/** How long the band offers to undo a press, in milliseconds (DR-54). */
const UNDO_MS = 10_000;

/**
 * The undo a press offered: the band's primary until `until`, and the pane's `u` until it is used
 * or replaced. A reload forgets it, as it forgets what a press showed.
 */
let offer: { handle: Handle; until: number } | null = null;

/**
 * Each `/room` run's full result, by the text it returned, so its `CommandOutput` row can draw it
 * whole; the newest few only, and a reload forgets them, so the row then draws its text.
 */
const drawn = new Map<string, { command: string; ran: Ran }>();
const DRAWN = 16;

/**
 * The temporary host's copy-out line (DR-53, DR-75), set from the first `temporary_host` warning a
 * run's reply carries and shown once: later warnings change nothing, and a reload forgets it.
 */
let copyOut: string | null = null;

async function setEngine($: $, state: EngineState): Promise<void> {
  // A host that failed keeps its one line, and nothing draws the engine.
  if (hostFailed) return;
  await update($, engineAtom, () => state);
  showStatus($, state);
}

/**
 * The status line for `state`: the engine's own line while it isn't ready, else the copy-out line,
 * else the line a refusal set, else none.
 */
function showStatus($: $, state: EngineState): void {
  $.ui.status(
    state.status !== "ready" && state.line !== null
      ? sanitize(state.line)
      : (copyOut ?? refusedNote ?? undefined),
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
  // The build's own start-up follows when it ends.
  if (building) return;
  watchStop();
  // The person key is made on the first start and kept in $.store across sessions (DR-72).
  await personKey($);
  // A host that failed at a first use while this ran stops its start-up here and at each step.
  if (hostFailed) return;
  // The host hands a mod its own directory; it doesn't set CLAUDE_PLUGIN_ROOT for mods (A-14, WS8).
  const root = $.plugin.root;
  if (isWindowsRoot(root)) {
    await setEngine($, unsupportedPlatform());
    return;
  }
  const resolver = `${root}/scripts/planroom`;
  await setEngine($, { ...INITIAL_ENGINE, resolver });
  const env = await clientEnv($);
  if (hostFailed) return;
  // A first resolve may download the engine, so it gets process.run's longest wait.
  let state = engineFromVersion(
    resolver,
    await runProcess($, [resolver, "version", "--json"], env, 600_000),
  );
  if (hostFailed) return;
  if (state.status === "ready") {
    state = {
      ...state,
      path: pathFromStatus(
        await runProcess($, [resolver, "status", "--json"], env, 30_000),
      ),
    };
  }
  await setEngine($, state);
  if (hostFailed) return;
  const bin = binary(state);
  if (state.status === "ready" && bin !== null) {
    // Writes a reload cut off go again first, so the pane the loop draws holds them.
    await resendOutbox($, bin);
  }
  if (state.status === "ready" && !hostFailed) {
    await refreshPane($);
    await registerOnce($);
  }
}

/** The engine's start-up in this module's life; session.start sets it, a reload starts it again. */
let starting: Promise<void> = Promise.resolve();

/** The writes this module life has running, so a second start-up never resends one mid-run. */
const running = new Set<string>();

/**
 * Run one catalogue command line through the engine; the reply as a tool result and its summary.
 * The one place every write starts (DR-8): a write, a leaf whose own flags take `--idem`, gets one
 * right after its path when the line has none, and goes in the outbox before its process starts,
 * by its argv, stdin, cause and the names of its variables, and leaves it on any reply, so a
 * reload mid-write resends it as the same request. A leaf that takes no `--idem`, such as `share`,
 * is no write the store can replay, so it runs as it is and never enters the outbox.
 */
async function runCommand(
  $: $,
  line: readonly string[],
  env: Record<string, string>,
  stdin?: string,
): Promise<Ran> {
  await starting;
  const engine = await read($, engineAtom);
  const bin = binary(engine);
  if (bin === null) {
    return { result: engineMissing(engine), summary: null };
  }
  const spec = leafOfArgv(line);
  let argv = [...line];
  let entry = null;
  if (spec !== undefined && takes(spec.name, "--idem")) {
    const words = spec.name.split(" ").length;
    const rest = line.slice(words);
    argv = [
      ...line.slice(0, words),
      ...supplied(spec.name, rest, undefined, crypto.randomUUID()),
      ...rest,
    ];
    entry = entryFor(argv, env, stdin, await $.session.id());
  }
  if (entry !== null) {
    const held = entry;
    running.add(held.idem);
    await update($, outboxAtom, (outbox) => add(outbox, held));
  }
  try {
    let ran = bin;
    let resolvedAgain = false;
    for (;;) {
      const run = await runProcess(
        $,
        [ran, ...argv],
        env,
        timeoutFor(argv),
        stdin,
      );
      const reply = parseReply(run.stdout);
      if (reply !== null) {
        if (copyOut === null) {
          const line = copyOutLine(reply.meta?.warnings);
          if (line !== null) {
            copyOut = line;
            // A host that failed while this ran keeps its one line.
            if (!hostFailed && (await read($, engineAtom)).status === "ready") {
              $.ui.status(copyOut);
            }
          }
        }
        return { result: toolResult(run), summary: replySummary(reply) };
      }
      // No envelope: the binary may be gone (a `cargo clean`, a removed release). Resolve again
      // once and run the line again on what that finds (DR-69).
      const exists = await $.fs.stat(ran).then(
        () => true,
        () => false,
      );
      if (afterNoReply(exists, resolvedAgain) === "resolve_again") {
        resolvedAgain = true;
        // The start-up resends no write of this module life that is in `running`, and this line's
        // entry is, so the second run below is the only one that sends it.
        starting = startEngine($).catch(() => undefined);
        await starting;
        const next = binary(await read($, engineAtom));
        // The same argv, so the same `--idem`, and the same env: the person's token was minted
        // for this line, not for the binary, and the entry stays in the outbox across both runs.
        if (next !== null) {
          ran = next;
          continue;
        }
        return {
          result: engineMissing(await read($, engineAtom)),
          summary: null,
        };
      }
      if (!exists) {
        const gone = engineGone(await read($, engineAtom), ran);
        await setEngine($, gone);
        return { result: engineMissing(gone), summary: null };
      }
      return { result: toolResult(run), summary: replySummary(null) };
    }
  } finally {
    if (entry !== null) {
      const idem = entry.idem;
      await update($, outboxAtom, (outbox) => clear(outbox, idem));
      running.delete(idem);
    }
  }
}

/**
 * Send again each write a reload cut off, before the watch loop starts (architecture.md,
 * "Wiring", step 3). It calls `runProcess` directly, since `runCommand` waits on the start-up
 * this runs inside. Each entry goes once, with its own argv, stdin and cause, as the same actor
 * and client, a person's with a fresh token; any reply clears it, and no resend adds an entry. An
 * entry of another session, or one the store answers `idempotency_mismatch`, is dropped, and the
 * status line says so once.
 */
async function resendOutbox($: $, bin: string): Promise<void> {
  const session = await $.session.id();
  let dropped = false;
  for (const entry of await read($, outboxAtom)) {
    // A write still running in this module life, or one another start-up is resending now, goes
    // once: never twice at the same time.
    if (running.has(entry.idem)) continue;
    running.add(entry.idem);
    if (!stale(entry, session)) {
      const again = resend(entry, session);
      let env = { ...(await engineEnv()), ...again.env };
      if (byPerson(entry)) {
        const words = leafOfArgv(again.argv)?.name.split(" ").length ?? 1;
        env = { ...(await personEnv($, again.argv, words)), ...again.env };
      }
      const run = await runProcess(
        $,
        [bin, ...again.argv],
        env,
        timeoutFor(again.argv),
        again.stdin,
      );
      if (toolResult(run).error?.code === "idempotency_mismatch")
        dropped = true;
    } else {
      dropped = true;
    }
    await update($, outboxAtom, (outbox) => clear(outbox, entry.idem));
    running.delete(entry.idem);
  }
  // A host that failed while these ran keeps its one line.
  if (dropped && !hostFailed) $.ui.status(MOD_STRINGS.outbox_dropped);
}

/**
 * Run a catalogue command as the person: a press, a submit or a `/room` they typed. It carries a
 * one-time token for exactly this line and never `PLANROOM_ACTOR` (DR-64). A write gets a fresh
 * `idem` for this action, and a leaf that needs a base gets the pane's `seq`, each right after
 * the leaf's path and only where its own flags take it (`supplied`).
 */
async function personRun(
  $: $,
  command: string,
  args: readonly string[],
): Promise<Ran> {
  const path = command.split(" ");
  const spec = leaf(command);
  const base =
    spec?.needs_base === true
      ? (await read($, paneAtom)).model?.focus.seq
      : undefined;
  const argv = [
    ...path,
    ...supplied(command, args, base, crypto.randomUUID()),
    ...args,
    "--json",
  ];
  const ran = await runCommand($, argv, await personEnv($, argv, path.length));
  // The store refused the person's own token: it may have lost the key, so check it again.
  if (ran.result.error?.code === "person_required") checked.delete(workspace);
  return ran;
}

/**
 * What a press does: a fallback's resolver step, the key's re-registration, putting the next
 * action back, or a command, whose result the pane shows in place of the press (ux-1).
 */
async function act(
  $: $,
  action: Action,
  from: "pane" | "band" = "pane",
  hotkey?: string,
): Promise<void> {
  // Any action key but the picker's own closes it first, then acts; its own deals it again.
  if (
    picker !== null &&
    !(action.kind === "run" && action.pick !== undefined)
  ) {
    if (closes(hotkey ?? "", PICKER_OPEN_KEYS[picker.name])) picker = null;
  }
  if (action.kind === "open") {
    const asked = await askPicker($, action.picker, project);
    picker = asked.picker;
    if (asked.error !== null) $.ui.toast(asked.error);
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "run" && action.pick !== undefined) {
    // A pick is the person's press: a read, whose result replaces the views with its one view, so
    // `z` comes back to the pane, not through what was shown before. The picker closes and the
    // decision returns; a refusal says why.
    picker = null;
    const seq = (await read($, paneAtom)).model?.focus.seq;
    const ran = await personRun($, action.command, action.args);
    const picked = seq === undefined ? null : viewFrom(action, ran, seq);
    if (action.pick === "project" && ran.result.ok) {
      project = projectOf(action.args);
    }
    if (picked === null || seq === undefined) {
      $.ui.toast(lineFor(action.command, action.label, ran));
    } else {
      views = [picked];
      view = {};
    }
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "clear") {
    shown = null;
    view = {};
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "select") {
    // A row key selects its row and runs nothing; an action key then acts on it (DR-54).
    view = { ...view, selected: action.row };
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "page") {
    view = { ...view, page: action.index };
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "up") {
    // `z` comes back one view; what was selected there is not what the view below draws.
    views = views.slice(0, -1);
    view = {};
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "run" && action.down === true) {
    // `x` goes down into the selected block through a read: no toast on success, since the
    // result draws in the list; a refusal says why.
    const seq = (await read($, paneAtom)).model?.focus.seq;
    const ran = await personRun($, action.command, action.args);
    const pushed = seq === undefined ? null : viewFrom(action, ran, seq);
    if (pushed === null || seq === undefined) {
      $.ui.toast(lineFor(action.command, action.label, ran));
    } else {
      views = [...current(views, seq), pushed];
      view = {};
    }
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "run" && action.command === "focus") {
    // Check again: a read the pane already runs, so no token, no toast and no way back to offer.
    shown = null;
    await refreshPane($);
    $.ui.invalidate("ui.render");
    return;
  }
  if (action.kind === "fallback" && action.run === "rekey") {
    await registerOnce($, true);
    return;
  }
  if (action.kind === "fallback") {
    if (action.run === "build") {
      // One build at a time: a press while one runs does nothing.
      if (building) return;
      const engine = await read($, engineAtom);
      if (engine.resolver !== null) {
        building = true;
        try {
          await setEngine($, {
            ...engine,
            status: "building",
            line: MOD_STRINGS.building_line,
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
              line: fill(MOD_STRINGS.build_failed, { said }),
            });
            return;
          }
        } finally {
          building = false;
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
  // An undo's own reply offers no undo, so the band never offers to undo an undo; a failed one,
  // such as a stale write, clears the offer too.
  if (action.undo === true) offer = null;
  const back = action.undo === true || !ran.result.ok ? null : undoOf(ran);
  if (back !== null) {
    offer = { handle: back, until: (await $.clock.now()) + UNDO_MS };
    $.clock.after(UNDO_MS, () => $.ui.invalidate("ui.render"));
  }
  const said = { label: sanitize(action.label), line };
  const warnings = warningsOf(ran.result).length;
  $.ui.toast(
    !ran.result.ok
      ? line
      : back !== null
        ? fill(
            from === "band"
              ? MOD_STRINGS.toast_undo
              : MOD_STRINGS.toast_undo_pane,
            said,
          )
        : warnings > 1
          ? fill(MOD_STRINGS.toast_warned, { ...said, count: warnings })
          : warnings === 1
            ? fill(MOD_STRINGS.toast_warned_one, said)
            : fill(MOD_STRINGS.toast, said),
  );
  shown = seq === undefined ? null : shownFrom(action, ran, seq);
  view = {};
  await refreshPane($);
  $.ui.invalidate("ui.render");
  await registerOnce($);
}

/** Rows as elements, sized by the layout to the body they draw in. */
function draw(
  $: $,
  e: RenderInput<"Pane" | "AbovePrompt">,
  rows: Row[],
  from: "pane" | "band" = "pane",
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
                  const press = (): Promise<void> =>
                    act($, b.action, from, b.hotkey);
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
    message: MOD_STRINGS.handler_failed,
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
    return { result: JSON.stringify(built.refused) };
  }
  const env: Record<string, string> = {
    ...(await engineEnv()),
    ...agentEnv(await $.session.id(), turn.turnId),
  };
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
      reason: MOD_STRINGS.guard_too_many_paths,
    };
  }
  const real = new Map<string, string[]>();
  for (const p of named) {
    real.set(p, await landing($, normalize(p, at.cwd, at.home)));
  }
  return guardCall(tool, input, (p) => real.get(p) ?? [], at, readOnly);
}

/**
 * Set by `session.start`'s host check, or by a member the check can't call missing at its first use
 * (`hostChanged`). While true every hook passes straight through, so the session is as with mods
 * off (DR-76, DR-129).
 */
let hostFailed = false;

/** The version the host check read, for the line a failure at a first use shows. */
let hostVersion: SessionVersion | null = null;

/**
 * A member the check took as present (`UNPROBED`) is missing at its first use, its call throwing a
 * TypeError: the host fails as a failed check does. Its one line shows once and every hook passes
 * through from now on. The watch loop ends and none starts again, so a `focus`, a tool call or a
 * `/room` run in flight ends without one, and a start-up under way stops at its next step.
 */
function hostChanged($: $, member: string): void {
  if (hostFailed) return;
  hostFailed = true;
  watchHalt();
  showHostLine($, changedLine(hostVersion, member));
  void starting.finally(watchStop).catch(() => undefined);
}

/**
 * The line the first refusal in this session start set (`refusedLine`): the status line while the
 * engine is ready, after the copy-out line, as the copy-out line is; null when nothing was refused.
 */
let refusedNote: string | null = null;

/**
 * Whether the host refused `/room`'s registration: its `command.run` hook then passes through, so a
 * command of that name, such as a built-in's, stays the host's.
 */
let roomRefused = false;

/**
 * A registration or a pane the host refused, with the error it rejected with: a refusal, not a
 * missing member, so the mod stays on and only what was refused is skipped (DR-129). The first in
 * a session start sets `host_refused`, naming what was refused and why, as the status line; later
 * ones change nothing.
 */
async function refused($: $, what: string, err: unknown): Promise<void> {
  if (hostFailed || refusedNote !== null) return;
  refusedNote = refusedLine(what, err);
  const engine = await read($, engineAtom);
  if (!hostFailed) showStatus($, engine);
}

/**
 * Asks for the pane: unasked at a start or an attach, never waiting for it to be placed, or focused
 * for a bare `/room`. A host without `ui.open` throws a TypeError, at the call or as the rejection,
 * which fails the host; any other rejection is a refusal, such as a `ui.open` hook's deny, which
 * leaves the pane closed and the mod on, and `/room` can open it later.
 */
async function openPane($: $, focus = false): Promise<void> {
  try {
    await $.ui.open(
      focus
        ? { id: PANE, title: TITLE, focus: true }
        : { id: PANE, title: TITLE },
    );
  } catch (err) {
    if (isMissing(err)) hostChanged($, "ui.open");
    else await refused($, MOD_STRINGS.host_refused_pane, err);
  }
}

// The host reads `register` off the bundle's source: a function declared and exported at the top
// level, its parameter spelled `on`. The bundle step keeps it that way (crates/xtask/src/modgen.rs).
export function register(on: Parameters<Register>[0]): void {
  checked = new Set();
  on("session.start", async ($, e, next) => {
    const host = await checkHost($);
    if (host.line !== null) {
      hostFailed = true;
      watchHalt();
      showHostLine($, host.line);
      return next(e);
    }
    hostFailed = false;
    hostVersion = host.version;
    watchResume();
    refusedNote = null;
    roomRefused = false;
    workspace = e.cwd;
    places = null;
    startEnv = null;
    gitFiles = new Map();
    // First, before any run can start: each names the data root the session starts with, the one
    // the guard holds, and waits for it to be read.
    pinDataRoot(placeEnv($).then(pinnedDataRoot));
    // The members the check can't call are probed by their first use here (DR-129). A TypeError
    // is a missing member: the host fails as a failed check does, and the session starts as with
    // mods off. Any other rejection is a refusal: only what was refused is skipped, the guard and
    // every other hook stay on, and one line names what was refused.
    const refusals: string[] = [];
    let reason: unknown;
    const use = async (
      member: string,
      what: string,
      call: () => Promise<unknown>,
    ): Promise<boolean> => {
      try {
        await call();
        return true;
      } catch (err) {
        if (isMissing(err)) {
          hostChanged($, member);
        } else {
          refusals.push(what);
          reason ??= err;
        }
        return false;
      }
    };
    // Immediate, so /room focuses the pane mid-turn, while the model is still working (A-22).
    roomRefused = !(await use("command.register", `/${ROOM_COMMAND.name}`, () =>
      $.command.register(ROOM_COMMAND),
    ));
    if (hostFailed) return next(e);
    // Awaited, so the tools are listed from turn one and the list never changes later (DR-20).
    for (const spec of toolSpecs()) {
      await use("tool.register", spec.name, () => $.tool.register(spec));
      if (hostFailed) return next(e);
    }
    if (refusals.length > 0) await refused($, refusals.join(", "), reason);
    // A cloud session can start with no surface; session.attach opens the pane then (A-11).
    if (e.surface !== null) void openPane($);
    if (hostFailed) return next(e);
    // The first prompt doesn't wait for the engine; commands and tools do. startEngine reports its
    // own failures through the engine atom, so a rejection here means the module was unloaded.
    starting = startEngine($).catch(() => undefined);
    return next(e);
  });

  on("session.attach", async ($, e, next) => {
    if (hostFailed) return next(e);
    void openPane($);
    return next(e);
  });

  on("command.run", { command: "room" }, async ($, e, next) => {
    // Registered before a later registration found a member missing, it passes through, as every
    // hook does then; and a /room the host refused is the host's, such as a built-in's.
    if (hostFailed || roomRefused) return next(e);
    const plan = roomPlan(e.args, roomCommands());
    if (plan.open) {
      // Bare /room focuses the pane and returns no text, so it costs the model nothing, and starts
      // no sync (DR-77).
      await openPane($, true);
      return hostFailed ? next(e) : {};
    }
    if (plan.run === undefined) {
      return plan.text === undefined ? {} : { text: plan.text };
    }
    const { command, args } = plan.run;
    // A /room the person typed acts as the person; one a plugin or the SDK ran doesn't.
    const typed = e.origin.kind === "composer" || e.origin.kind === "bridge";
    const ran: Ran = {
      ...(typed
        ? await personRun($, command, args)
        : await runCommand(
            $,
            [...command.split(" "), ...args, "--json"],
            await clientEnv($),
          )),
      args,
    };
    await refreshPane($);
    await registerOnce($);
    // The model reads the summary line and handles; the row draws the whole result (DR-65).
    const text = roomText(command, ran);
    drawn.delete(text);
    drawn.set(text, { command, ran });
    for (const old of drawn.keys()) {
      if (drawn.size <= DRAWN) break;
      drawn.delete(old);
    }
    // `/room share` hands the model the publish's arguments, so it publishes with no other read.
    const context = shareContext(command, ran);
    return context === undefined ? { text } : { text, context };
  });

  // One stable section, the same bytes in every state, appended last (DR-20).
  on("prompt.compose", async ($, e, next) => {
    if (hostFailed) return next(e);
    const composed = await next(e);
    return { sections: composeSections(composed.sections) };
  });

  // The main loop's turn: its id is each tool write's cause, and the base resets with it.
  on("turn.start", async ($, e, next) => {
    if (hostFailed) return next(e);
    if (e.agentId === undefined) {
      await update($, turnSeqAtom, () => ({ turnId: e.turnId, seq: null }));
    }
    return next(e);
  });

  on("prompt.autocomplete", async ($, e, next) => {
    if (hostFailed) return next(e);
    // A refused /room isn't the mod's, so it completes nothing for it.
    const mine = roomRefused
      ? []
      : completions(e.text, e.cursor, roomCommands());
    const below = await next(e);
    return mine.length === 0
      ? below
      : { suggestions: [...below.suggestions, ...mine] };
  });

  // A handler that throws still answers with the one tool-result shape, never a bare failure. On a
  // failed host the hook only passes through, so what lies beneath answers, its rejection
  // included: `next` replays it, and the handler's own answer never takes its place.
  on("tool.call", { tool: "mcp__planroom__room_query" }, ($, e, next) =>
    hostFailed
      ? next(e)
      : handleTool($, "room_query", e as unknown as ToolInput),
  ).catch(($, e, next) => (hostFailed ? next(e) : { result: HANDLER_FAILED }));
  on("tool.call", { tool: "mcp__planroom__room_change" }, ($, e, next) =>
    hostFailed
      ? next(e)
      : handleTool($, "room_change", e as unknown as ToolInput),
  ).catch(($, e, next) => (hostFailed ? next(e) : { result: HANDLER_FAILED }));
  on("tool.call", { tool: "mcp__planroom__room_handoff" }, ($, e, next) =>
    hostFailed
      ? next(e)
      : handleTool($, "room_handoff", e as unknown as ToolInput),
  ).catch(($, e, next) => (hostFailed ? next(e) : { result: HANDLER_FAILED }));

  // The guard, for every tool call of every agent. Planroom's reads are confined to the workspace,
  // and an agent's edit, intent, claim and curate writes are ghosts or proposals, so those need no
  // prompt (DR-52); everything else the guard lets through goes on to the session's own rules.
  on("tool.check", async ($, e, next) => {
    if (hostFailed) return next(e);
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
          reason: MOD_STRINGS.guard_unchecked,
        },
  );

  on(
    "ui.render",
    { component: "Pane", requestId: PANE },
    async ($, e, next) => {
      if (hostFailed) return next(e);
      const engine = await read($, engineAtom);
      const pane = await read($, paneAtom);
      const columns = e.props.bodyColumns;
      const rows =
        engine.status === "ready" && pane.model !== null
          ? paneRows(
              pane.model,
              columns,
              e.props.scroll.bodyRows,
              shown,
              viewFor(pane.model.focus.seq, pane.model.focus.level),
              offer?.handle ?? null,
              views,
              picker,
            )
          : fallbackRows(fallbackFor(engine, pane.error), columns);
      return draw($, e, rows);
    },
  );

  // A `/room` run's row draws its whole result in place of the text the model reads (DR-65).
  on(
    "ui.render",
    { component: "CommandOutput", props: { command: "room" } },
    ($, e, next) => {
      if (hostFailed) return next(e);
      const held = drawn.get(e.props.text);
      return held === undefined
        ? next(e)
        : drawRoom($, e, roomRows(held.command, held.ran));
    },
  );

  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    if (hostFailed) return next(e);
    if (e.props.hasSurvey) {
      return next(e);
    }
    const engine = await read($, engineAtom);
    const pane = await read($, paneAtom);
    const source =
      engine.status === "ready" && pane.model !== null
        ? { model: pane.model }
        : { fallback: fallbackFor(engine, pane.error) };
    const now = await $.clock.now();
    const undo = offer !== null && now < offer.until ? offer.handle : null;
    return draw(
      $,
      e,
      [bandRow(source, e.props.bodyColumns, shown, undo)],
      "band",
    );
  });
}
