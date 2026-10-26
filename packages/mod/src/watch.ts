/**
 * The watch loop's pure parts (architecture.md, "Wiring" step 4, and "Live within 1 s"): reading
 * `planroom watch --json` NDJSON from the pieces `$.process.spawn` streams, the refetch schedule for
 * `focus --json`, and the wait before a dead child is started again. register.tsx runs the child,
 * the clock and the refetch.
 */

import type { PaneModel, StreamFrame } from "@planroom/contract";

import type { Reply } from "./engine";

/** The mod refetches on the first frame of a burst, then after this much quiet... */
export const QUIET_MS = 250;
/** ...and at least this often while frames keep coming. */
export const MAX_WAIT_MS = 500;

/** One line of `watch --json`: a stream frame, or the one error envelope a failed start prints. */
export type WatchLine =
  | { kind: "frame"; frame: StreamFrame }
  | { kind: "reply"; reply: Reply }
  | { kind: "other" };

/** Parse one NDJSON line. Anything that isn't a frame or an envelope is ignored, never fatal. */
export function parseLine(line: string): WatchLine {
  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch {
    return { kind: "other" };
  }
  if (typeof value !== "object" || value === null) {
    return { kind: "other" };
  }
  const v = value as Record<string, unknown>;
  if (typeof v["type"] === "string") {
    return { kind: "frame", frame: value as StreamFrame };
  }
  if (typeof v["ok"] === "boolean") {
    return { kind: "reply", reply: value as Reply };
  }
  return { kind: "other" };
}

/**
 * Joins the pieces a spawned child writes, which aren't split at newlines, into whole lines.
 * A partial last line waits for the next piece.
 */
export class LineReader {
  private rest = "";

  push(text: string): string[] {
    const joined = this.rest + text;
    const lines = joined.split("\n");
    this.rest = lines.pop() ?? "";
    return lines.map((l) => l.trim()).filter((l) => l !== "");
  }

  /** What's left when the child ended without a final newline. */
  flush(): string[] {
    const last = this.rest.trim();
    this.rest = "";
    return last === "" ? [] : [last];
  }
}

/** The `seq` a frame moves the loop's cursor to, or null when it moves nothing. */
export function frameSeq(frame: StreamFrame): number | null {
  switch (frame.type) {
    case "event":
    case "reset":
      return frame.seq;
    default:
      return null;
  }
}

/**
 * The loop's cursor after a batch of frames, from `held`: a reset names the seq to refetch from,
 * and an event only moves the cursor on. A restarted child streams from the result (`--since`).
 */
export function advanceCursor(
  held: number | null,
  frames: readonly StreamFrame[],
): number | null {
  return frames.reduce<number | null>((at, f) => {
    const seq = frameSeq(f);
    if (seq === null) return at;
    if (f.type === "reset") return seq;
    return at === null ? seq : Math.max(at, seq);
  }, held);
}

/** Whether a frame changes what `focus` would draw; a ping only says the stream is alive. */
export function frameRefreshes(frame: StreamFrame): boolean {
  return frame.type !== "ping";
}

/** The refetch schedule's state: when the last frame came, when focus last ran, and whether a run is owed. */
export interface Debounce {
  lastFrame: number | null;
  lastRun: number | null;
  pending: boolean;
}

export const IDLE: Debounce = {
  lastFrame: null,
  lastRun: null,
  pending: false,
};

/** What to do now: the next state, whether to run `focus` now, and when to look again. */
export interface Step {
  state: Debounce;
  run: boolean;
  wakeAt: number | null;
}

function settle(s: Debounce, now: number): Step {
  const last = s.lastFrame ?? now;
  const due = Math.min(last + QUIET_MS, (s.lastRun ?? last) + MAX_WAIT_MS);
  if (now >= due) {
    return {
      state: { ...s, pending: false, lastRun: now },
      run: true,
      wakeAt: null,
    };
  }
  return { state: s, run: false, wakeAt: due };
}

/**
 * A frame arrived at `now`. The first frame after a quiet spell runs `focus` at once (the leading
 * edge); later ones owe a run, due after `QUIET_MS` of quiet or `MAX_WAIT_MS` after the last run,
 * whichever is sooner, so the pane is never more than half a second behind under load.
 */
export function onFrame(s: Debounce, now: number): Step {
  const quiet = s.lastFrame === null || now - s.lastFrame >= QUIET_MS;
  if (!s.pending && quiet) {
    return {
      state: { lastFrame: now, lastRun: now, pending: false },
      run: true,
      wakeAt: null,
    };
  }
  return settle({ ...s, lastFrame: now, pending: true }, now);
}

/** A wake-up at `now`: runs the owed refetch if it is due, else says when to look again. */
export function onTick(s: Debounce, now: number): Step {
  return s.pending ? settle(s, now) : { state: s, run: false, wakeAt: null };
}

/**
 * How long to wait before starting `watch` again after it ended: 1 s after a child that streamed
 * frames, then doubling up to 30 s while it keeps failing, so a broken engine never spins.
 */
export function restartDelay(failures: number): number {
  if (failures <= 0) return 1_000;
  return Math.min(30_000, 1_000 * 2 ** failures);
}

/**
 * Whether `focus` answered from a store. Outside a workspace it returns its not-yet-a-workspace
 * empty state at seq 0, and the mod starts no watch until the first scan makes a store (DR-57).
 */
export function hasStore(model: PaneModel): boolean {
  return model.empty === null || model.focus.seq > 0;
}

/** Error codes after which the loop stops instead of starting the child again. */
export function stopsTheLoop(code: string): boolean {
  // No store yet: the first scan starts the loop (DR-57). A newer engine migrated the store: the
  // mod shows one line and never restarts the old binary, so nothing crash-loops (DR-53).
  return code === "workspace_not_found" || code === "schema_mismatch";
}
