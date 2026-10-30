/**
 * The watch loop under `claude plugin test`: `process.spawn` answers as `planroom watch --json`
 * would, from the golden frames, on a mocked clock that also drives the refetch schedule.
 */

import { expect, test } from "claude-code/testing";

import frames from "../../../tests/golden/cli/watch-frames.json";
import {
  ENGINE,
  focusRuns,
  host,
  NO_STORE,
  PANE,
  readyEngine,
  SESSION,
  START,
  type Piece,
} from "./host";

const NDJSON = frames.map((f) => JSON.stringify(f)).join("\n") + "\n";
/** The last seq the golden frames carry, where a restarted watch picks up. */
const LAST = Math.max(...frames.map((f) => (f as { seq?: number }).seq ?? 0));
const SLOW = { timeoutMs: 20_000 };

function event(seq: number): string {
  return (
    JSON.stringify({
      type: "event",
      seq,
      kind: "edit",
      target: null,
      project: null,
      actor: { kind: "system", part: "scan" },
      status: "applied",
      summary: `event ${seq}`,
    }) + "\n"
  );
}

function watchArgv(since?: string): string[] {
  const argv = [ENGINE, "watch", "--json"];
  if (since !== undefined) argv.push("--since", since);
  return [...argv, "--session", SESSION];
}

test(
  "watch starts once focus answers from a store, and its frames refetch focus",
  SLOW,
  async ($, on) => {
    const h = host(on, readyEngine, {
      watch: () => [{ after: 100, text: NDJSON }],
    });
    await $.session.start(START);
    await h.clock.settle();

    expect(h.children).toHaveLength(1);
    expect(h.children[0]?.argv).toEqual(watchArgv());
    expect(h.children[0]?.env["PLANROOM_CLIENT"]).toBe(`mod:${SESSION}`);
    const before = focusRuns(h).length;
    await h.clock.advance(100);
    expect(focusRuns(h).length).toBe(before + 1);
  },
);

test(
  "watch_restarts_after_a_reload: from the last seq it read, never from the start",
  SLOW,
  async ($, on) => {
    const scripts: Piece[][] = [
      // The golden frames end at LAST; then the child dies, as an engine restart kills it.
      [
        { after: 10, text: NDJSON },
        { after: 10, end: { code: null, signal: "SIGTERM" } },
      ],
      // The restarted child reads one more event, then streams on until a reload kills it.
      [{ after: 10, text: event(LAST + 1) }],
    ];
    const h = host(on, readyEngine, { watch: (nth) => scripts[nth] ?? [] });
    await $.session.start(START);
    await h.clock.settle();
    await h.clock.advance(20);
    expect(h.children).toHaveLength(1);
    // The loop waits 1 s after a child that streamed, then starts again from the last seq.
    await h.clock.advance(1_000);
    expect(h.children).toHaveLength(2);
    expect(h.children[1]?.argv).toEqual(watchArgv(String(LAST)));
    await h.clock.advance(10);

    // A reload runs session.start again, $.state kept: one new child, from the seq it kept.
    await $.session.start(START);
    await h.clock.settle();
    expect(h.children).toHaveLength(3);
    expect(h.children[2]?.argv).toEqual(watchArgv(String(LAST + 1)));
    // The replaced loop is gone: nothing starts a fourth child.
    await h.clock.advance(5_000);
    expect(h.children).toHaveLength(3);
  },
);

test(
  "a watch that refuses for want of a store isn't started again",
  SLOW,
  async ($, on) => {
    const h = host(on, readyEngine, {
      watch: () => [
        { after: 0, text: NO_STORE },
        { after: 0, end: { code: 4, signal: null } },
      ],
    });
    await $.session.start(START);
    await h.clock.settle();
    await h.clock.advance(60_000);
    expect(h.children).toHaveLength(1);
  },
);

test(
  "a child that keeps failing backs off instead of spinning",
  SLOW,
  async ($, on) => {
    // Each child dies at once with nothing streamed, as an engine that crashes on start would.
    const h = host(on, readyEngine, {
      watch: () => [{ after: 0, end: { code: 1, signal: null } }],
    });
    await $.session.start(START);
    await h.clock.settle();
    await h.clock.advance(31_000);
    // Restarts after 2, 4, 8 and 16 s: five children in 31 s, not hundreds.
    expect(h.children).toHaveLength(5);
  },
);

test(
  "debounce_max_wait: a steady stream refetches focus at least every 500 ms, then once after 250 ms of quiet",
  SLOW,
  async ($, on) => {
    // 21 frames, one every 100 ms over 2 s.
    const burst: Piece[] = Array.from({ length: 21 }, (_, i) => ({
      after: 100,
      text: event(500 + i),
    }));
    const h = host(on, readyEngine, { watch: () => burst });
    await $.session.start(START);
    await h.clock.settle();
    const before = focusRuns(h).length;
    const t0 = h.clock.now();
    await h.clock.advance(3_500);

    const runs = focusRuns(h)
      .slice(before)
      .map((at) => at - t0);
    // The first frame, at 100 ms, refetches at once; then at most 500 ms apart; far fewer refetches
    // than frames; and the last 250 ms after the last frame, at 2,100 ms.
    expect(runs[0]).toBe(100);
    for (let i = 1; i < runs.length; i++) {
      expect((runs[i] ?? 0) - (runs[i - 1] ?? 0)).toBeLessThanOrEqual(500);
    }
    expect(runs.length).toBeGreaterThanOrEqual(5);
    expect(runs.length).toBeLessThan(10);
    expect(runs.at(-1)).toBe(2_100 + 250);
  },
);

test(
  "a_presence_frame_sets_the_who_line_and_runs_no_focus",
  SLOW,
  async ($, on) => {
    const row = (id: string, client: string) => ({
      actor: { kind: "person", id, name: null },
      surface: "mod",
      client_id: client,
      doing: null,
      focus: null,
      seen_at: "2026-01-01T00:00:00.000000Z",
    });
    const frame =
      JSON.stringify({
        type: "presence",
        actors: [row("you", `mod:${SESSION}`), row("ada", "mod:other")],
      }) + "\n";
    const h = host(on, readyEngine, {
      watch: () => [{ after: 100, text: frame }],
    });
    await $.session.start(START);
    await h.clock.settle();
    const before = focusRuns(h).length;
    await h.clock.advance(100);
    await h.clock.settle();
    // The frame carries the whole set, so no focus ran; the pane's who line names the other
    // person and leaves out this session's own row.
    expect(focusRuns(h).length).toBe(before);
    const ui = await $.ui.mount({ ...PANE, surface: "terminal" });
    const who = await ui.find({ key: "who-0" });
    expect(who?.text).toContain("ada");
    expect(who?.text).not.toContain("you");
    await ui.unmount();
  },
);
