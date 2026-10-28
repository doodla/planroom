/**
 * Running the engine, and the watch loop (architecture.md, "Wiring" step 4; DR-6):
 * `planroom watch --json` through `$.process.spawn` in a detached loop, each frame moving
 * `lastSeq` and scheduling a `focus` refetch, and the child started again from `lastSeq` when it
 * ends. A reload kills the child and the loop with the module; `session.start` then starts a new
 * one from the `lastSeq` `$.state` kept.
 *
 * The host reads `$` only as `$.noun.call(...)` or passed to a function declared in the module,
 * so this file is plain functions over module state, never methods that take `$`.
 */

import { read, update } from "claude-code";
import type {
  EngineInterface,
  HookStream,
  ProcessSpawnChunk,
  ProcessSpawnResult,
  Timer,
} from "claude-code";
import type { Envelope, PaneModel, StreamFrame } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

import { engineAtom, lastSeqAtom, paneAtom } from "./atoms";
import { parseReply, type RunResult } from "./engine";
import { UNPLACED } from "./guard";
import type { EngineState, PaneState } from "./state-types";
import {
  advanceCursor,
  frameRefreshes,
  frameSeq,
  hasStore,
  IDLE,
  LineReader,
  onFrame,
  onTick,
  parseLine,
  restartDelay,
  stopsTheLoop,
  type Debounce,
  type Step,
} from "./watch";

type $ = EngineInterface;
type Child = HookStream<ProcessSpawnChunk, ProcessSpawnResult>;

/** Run one process; a refusal to start reads as a failed run, never a throw. */
export async function runProcess(
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

/**
 * The variables that pick the engine binary, set empty on every run the mod starts, the resolver's
 * and the engine's alike: the resolver reads an empty one as unset, so a value the session
 * inherited, such as one a shell's start-up file or a settings file set, never picks what a
 * person's press runs (DR-64, DR-69).
 */
const ENGINE_UNPICKED: Readonly<Record<string, string>> = {
  PLANROOM_ENGINE: "",
  PLANROOM_RELEASE_BASE_URL: "",
};

/**
 * Raw output, set empty on every run too: the engine reads an empty one as unset, so a
 * `PLANROOM_RAW=1` the session inherited never shows a tool result, `/room` or the pane unmasked.
 * It is for the truth comparator alone, never for what the mod shows (DR-75).
 */
const REDACTION_KEPT: Readonly<Record<string, string>> = { PLANROOM_RAW: "" };

/**
 * The engine's test-only seams, set empty on every run as well: a debug engine reads
 * `PLANROOM_TEST_OPENER` as the program that opens the login page, and `PLANROOM_TEST_EUID` as the
 * user the repo-ownership check compares owners with, and an empty value of either as none, so a
 * value the session inherited never picks what a press of `open` runs or which repos count as
 * another user's (DR-78, DR-92).
 */
const TEST_SEAMS_OFF: Readonly<Record<string, string>> = {
  PLANROOM_TEST_OPENER: "",
  PLANROOM_TEST_EUID: "",
};

/**
 * The data root this session placed at its start (guard.ts `pinnedDataRoot`), as a promise the
 * session's start sets before anything else runs, so no run can read it before it's placed.
 */
let pinnedData: Promise<string> = Promise.resolve(UNPLACED);

/**
 * Pin the data root every run the mod starts names from now on: the resolver picks the engine
 * cache in it, so a `PLANROOM_DATA`, `XDG_DATA_HOME` or `HOME` the session's environment takes on
 * later, such as from a settings file's `env`, never moves what a person's press runs.
 */
export function pinDataRoot(root: Promise<string>): void {
  pinnedData = root;
}

/** What every run the mod starts sets, the resolver's and the engine's alike, so it picks the engine itself. */
export async function engineEnv(): Promise<Record<string, string>> {
  return {
    ...ENGINE_UNPICKED,
    ...REDACTION_KEPT,
    ...TEST_SEAMS_OFF,
    PLANROOM_DATA: await pinnedData,
  };
}

/** The writer instance every engine run names: this mod session (DR-60). */
export async function clientEnv($: $): Promise<Record<string, string>> {
  return {
    ...(await engineEnv()),
    PLANROOM_CLIENT: `mod:${await $.session.id()}`,
  };
}

/**
 * The engine binary, once resolved. A person key the workspace doesn't know stops presses from
 * acting as the person, not the engine, so reads, tools and the watch loop still run.
 */
export function binary(engine: EngineState): string | null {
  return engine.status === "ready" || engine.status === "person_key_mismatch"
    ? (engine.path ?? engine.resolver)
    : null;
}

/**
 * Run `focus --json` and keep its PaneModel, or its error, for the pane and the band; once the
 * workspace has a store, make sure the watch loop runs.
 */
export async function refreshPane($: $): Promise<void> {
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
  if (pane.model !== null && hasStore(pane.model)) {
    watchEnsure($);
  }
}

/** The loop's state in this module's life. A start or stop bumps `generation`; an older loop ends. */
let generation = 0;
let running = false;
let child: Child | null = null;
let debounce: Debounce = IDLE;
let timer: Timer | null = null;
let refreshing = false;
let owed = false;

/** Start the loop, ending any loop this module already runs. */
export function watchStart($: $): void {
  watchStop();
  const gen = generation;
  running = true;
  void watchLoop($, gen)
    .catch(() => undefined)
    .finally(() => {
      if (gen === generation) running = false;
    });
}

/** Start the loop unless one runs. */
export function watchEnsure($: $): void {
  if (!running) watchStart($);
}

/** End the loop and its child. */
export function watchStop(): void {
  generation += 1;
  running = false;
  timer?.cancel();
  timer = null;
  debounce = IDLE;
  const old = child;
  child = null;
  if (old !== null) {
    void old.return({ code: null, signal: "SIGTERM" }).catch(() => undefined);
  }
}

async function watchLoop($: $, gen: number): Promise<void> {
  let failures = 0;
  while (gen === generation) {
    const bin = binary(await read($, engineAtom));
    if (bin === null) return;
    const { seq } = await read($, lastSeqAtom);
    const argv = [bin, "watch", "--json"];
    if (seq !== null) argv.push("--since", String(seq));
    argv.push("--session", await $.session.id());
    const env = await clientEnv($);
    if (gen !== generation) return;
    const stream = $.process.spawn({ argv, env });
    child = stream;
    const reader = new LineReader();
    let streamed = false;
    let refusal: Envelope | null = null;
    try {
      for (;;) {
        const piece = await stream.next();
        if (gen !== generation) return;
        if (piece.done === true) break;
        if (piece.value.stream !== "stdout") continue;
        const got = await readLines($, gen, reader.push(piece.value.text));
        streamed ||= got.frames > 0;
        refusal = got.refusal ?? refusal;
      }
      const got = await readLines($, gen, reader.flush());
      streamed ||= got.frames > 0;
      refusal = got.refusal ?? refusal;
    } catch {
      // The child couldn't start, or died under the loop: start it again below.
    }
    if (gen !== generation) return;
    child = null;
    const code = refusal?.error?.code;
    if (refusal !== null && code !== undefined && stopsTheLoop(code)) {
      // No store yet stays silent: the pane shows focus's not-yet-a-workspace state, and the first
      // scan starts the loop. Anything else is one status line, never a restart (DR-53).
      if (code !== "workspace_not_found") {
        const said = refusal.error?.hint ?? refusal.error?.message;
        $.ui.status(said === undefined ? undefined : sanitize(said));
      }
      return;
    }
    failures = streamed ? 0 : failures + 1;
    await $.clock.sleep(restartDelay(failures));
  }
}

/** Read whole lines: move `lastSeq` past their frames and schedule the refetch. */
async function readLines(
  $: $,
  gen: number,
  lines: string[],
): Promise<{ frames: number; refusal: Envelope | null }> {
  const frames: StreamFrame[] = [];
  let refusal: Envelope | null = null;
  for (const line of lines) {
    const parsed = parseLine(line);
    if (parsed.kind === "frame") frames.push(parsed.frame);
    if (parsed.kind === "reply" && !parsed.reply.ok) refusal = parsed.reply;
  }
  if (frames.some((f) => frameSeq(f) !== null)) {
    await update($, lastSeqAtom, (held) => ({
      seq: advanceCursor(held.seq, frames),
    }));
  }
  if (gen === generation && frames.some(frameRefreshes)) {
    // The frames of one piece arrive at one moment, so they count as one for the schedule.
    const now = await $.clock.now();
    applyStep($, onFrame(debounce, now), now);
  }
  return { frames: frames.length, refusal };
}

function applyStep($: $, step: Step, now: number): void {
  debounce = step.state;
  if (step.run) refetch($);
  if (step.wakeAt !== null) {
    timer?.cancel();
    const gen = generation;
    timer = $.clock.after(Math.max(0, step.wakeAt - now), () => {
      if (gen === generation) void tick($);
    });
  }
}

async function tick($: $): Promise<void> {
  const now = await $.clock.now();
  applyStep($, onTick(debounce, now), now);
}

/** One `focus` at a time; a run asked for meanwhile follows when it ends. */
function refetch($: $): void {
  if (refreshing) {
    owed = true;
    return;
  }
  refreshing = true;
  void refreshPane($)
    .catch(() => undefined)
    .finally(() => {
      refreshing = false;
      if (owed) {
        owed = false;
        refetch($);
      }
    });
}
