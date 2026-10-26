/**
 * The real engine's `watch --json` through the mod's own watch-loop functions: the pieces the
 * child wrote, as `$.process.spawn` would hand them over, go through `LineReader`, `parseLine`,
 * `advanceCursor` and the refetch schedule, and the two must agree on every line. The run itself
 * happens in the vitest global setup, `tests/e2e/src/engine-watch.ts`, since the mod has no Node
 * APIs; it is skipped, saying why, when `target/debug/planroom` isn't built.
 */

import { expect, inject, test } from "vitest";
import type { Envelope, StreamFrame } from "@planroom/contract";

import {
  advanceCursor,
  frameRefreshes,
  frameSeq,
  IDLE,
  LineReader,
  onFrame,
  onTick,
  parseLine,
  stopsTheLoop,
  type Debounce,
} from "./watch";

/** What the global setup provides; `tests/e2e/src/engine-watch.ts` declares the same shape. */
interface WatchRun {
  skipped: string | null;
  since: number;
  chunks: { at: number; text: string }[];
  scans: number[];
  exitCode: number | null;
}

declare module "vitest" {
  export interface ProvidedContext {
    watchRun: WatchRun;
  }
}

const run = inject("watchRun");
const NAME =
  "the engine's watch --json reads through the mod's watch loop, line for line";

// A skipped run names its reason in the test's name, which the reporter prints.
test.skipIf(run.skipped !== null)(
  run.skipped === null ? NAME : `${NAME} (skipped: ${run.skipped})`,
  () => {
    const reader = new LineReader();
    const frames: StreamFrame[] = [];
    const replies: Envelope[] = [];
    const others: string[] = [];
    let cursor: number | null = run.since;
    let schedule: Debounce = IDLE;
    let refetches = 0;
    let last = 0;
    for (const chunk of run.chunks) {
      const batch: StreamFrame[] = [];
      for (const line of reader.push(chunk.text)) {
        const parsed = parseLine(line);
        if (parsed.kind === "frame") batch.push(parsed.frame);
        else if (parsed.kind === "reply") replies.push(parsed.reply);
        else others.push(line);
      }
      frames.push(...batch);
      cursor = advanceCursor(cursor, batch);
      if (batch.some(frameRefreshes)) {
        const step = onFrame(schedule, chunk.at);
        schedule = step.state;
        if (step.run) refetches++;
      }
      last = chunk.at;
    }
    const trailing = onTick(schedule, last + 10_000);
    if (trailing.run) refetches++;
    expect(reader.flush()).toEqual([]);

    // Every line is a frame or the one closing envelope; nothing is noise.
    expect(others).toEqual([]);
    // A ping first, then each event after --since once, in order: the replay, then the rescan.
    expect(frames[0]).toEqual({ type: "ping" });
    const events = frames.filter((f) => f.type === "event");
    const final = run.scans.at(-1) ?? 0;
    const want = Array.from(
      { length: final - run.since },
      (_, i) => run.since + 1 + i,
    );
    expect(events.map(frameSeq)).toEqual(want);
    expect(
      frames.filter((f) => f.type !== "event" && f.type !== "ping"),
    ).toEqual([]);
    for (const e of events) {
      expect(e).toMatchObject({
        kind: "scan",
        status: "applied",
        actor: { kind: "system", part: "scan" },
      });
      expect(
        e.type === "event" && e.summary.length > 0 && e.summary.length <= 2048,
      ).toBe(true);
    }
    // The mod's cursor ends where the engine's last commit is, and a restart would pass it on.
    expect(cursor).toBe(final);
    // The frames refetched focus, and far less often than one per frame would.
    expect(refetches).toBeGreaterThan(0);
    expect(refetches).toBeLessThanOrEqual(events.length);

    // The migration ends the stream with one schema_mismatch envelope, which stops the loop.
    expect(replies).toHaveLength(1);
    const reply = replies[0] as Envelope;
    expect(reply.ok).toBe(false);
    expect(reply.command).toBe("watch");
    expect(reply.error?.code).toBe("schema_mismatch");
    expect(stopsTheLoop(reply.error?.code ?? "")).toBe(true);
    expect(run.exitCode).toBe(7);
  },
);
