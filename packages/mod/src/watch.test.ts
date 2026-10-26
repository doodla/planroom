import { expect, test } from "vitest";

import frames from "../../../tests/golden/cli/watch-frames.json";
import {
  frameRefreshes,
  frameSeq,
  IDLE,
  LineReader,
  MAX_WAIT_MS,
  onFrame,
  onTick,
  parseLine,
  QUIET_MS,
  restartDelay,
  stopsTheLoop,
  type Debounce,
} from "./watch";

test("pieces of NDJSON become whole lines, a partial line waits", () => {
  const r = new LineReader();
  expect(r.push('{"type":"ping"}\n{"type":"ev')).toEqual(['{"type":"ping"}']);
  expect(r.push('ent","seq":3}\n\n')).toEqual(['{"type":"event","seq":3}']);
  expect(r.push('{"type":"reset","seq":9}')).toEqual([]);
  expect(r.flush()).toEqual(['{"type":"reset","seq":9}']);
  expect(r.flush()).toEqual([]);
});

test("the golden frames parse, and each moves the cursor as its kind says", () => {
  const parsed = frames.map((f) => parseLine(JSON.stringify(f)));
  expect(parsed.every((p) => p.kind === "frame")).toBe(true);
  const seqs = parsed.map((p) => (p.kind === "frame" ? frameSeq(p.frame) : -1));
  expect(seqs.filter((s) => s !== null).length).toBeGreaterThan(0);
  const ping = parsed.find(
    (p) => p.kind === "frame" && p.frame.type === "ping",
  );
  if (ping?.kind === "frame") {
    expect(frameSeq(ping.frame)).toBeNull();
    expect(frameRefreshes(ping.frame)).toBe(false);
  }
});

test("an error envelope and noise are told apart from frames", () => {
  expect(
    parseLine(
      '{"schema_version":1,"ok":false,"command":"watch","error":{"code":"workspace_not_found","message":"x"},"meta":{"seq":0}}',
    ),
  ).toMatchObject({ kind: "reply", reply: { ok: false } });
  expect(parseLine("not json")).toEqual({ kind: "other" });
  expect(parseLine("[1]")).toEqual({ kind: "other" });
  expect(stopsTheLoop("workspace_not_found")).toBe(true);
  expect(stopsTheLoop("schema_mismatch")).toBe(true);
  expect(stopsTheLoop("store_busy")).toBe(false);
});

/** Drive the schedule with frames at `times`, waking whenever it asks; the times focus ran. */
function runs(times: number[], end: number): number[] {
  let s: Debounce = IDLE;
  let wake: number | null = null;
  const out: number[] = [];
  const queue = [...times];
  for (let now = 0; now <= end; now++) {
    if (wake !== null && now >= wake) {
      const step = onTick(s, now);
      s = step.state;
      wake = step.wakeAt;
      if (step.run) out.push(now);
    }
    while (queue[0] === now) {
      queue.shift();
      const step = onFrame(s, now);
      s = step.state;
      wake = step.wakeAt ?? wake;
      if (step.run) out.push(now);
    }
  }
  return out;
}

test("a lone frame refetches at once and owes nothing", () => {
  expect(runs([100], 2_000)).toEqual([100]);
});

test("a burst refetches on its first frame and after 250 ms of quiet", () => {
  expect(runs([0, 50, 120], 2_000)).toEqual([0, 120 + QUIET_MS]);
});

test("debounce_max_wait: frames every 100 ms refetch at least every 500 ms, then trail", () => {
  const frameTimes = Array.from({ length: 21 }, (_, i) => i * 100);
  const got = runs(frameTimes, 4_000);
  expect(got[0]).toBe(0);
  for (let i = 1; i < got.length; i++) {
    expect((got[i] ?? 0) - (got[i - 1] ?? 0)).toBeLessThanOrEqual(MAX_WAIT_MS);
  }
  expect(got.at(-1)).toBe(2_000 + QUIET_MS);
});

test("a dead child restarts after 1 s, backing off to 30 s", () => {
  expect(restartDelay(0)).toBe(1_000);
  expect(restartDelay(1)).toBe(2_000);
  expect(restartDelay(3)).toBe(8_000);
  expect(restartDelay(10)).toBe(30_000);
});
