import { expect, test } from "vitest";
import type { PaneModel, StreamFrame } from "@planroom/contract";

import focusNoRepos from "../../../tests/golden/cli/focus-no-repos.json";
import focusPlanOnly from "../../../tests/golden/cli/focus-plan-only.json";
import frames from "../../../tests/golden/cli/watch-frames.json";
import {
  advanceCursor,
  frameRefreshes,
  frameSeq,
  hasStore,
  IDLE,
  LineReader,
  MAX_WAIT_MS,
  onFrame,
  onTick,
  parseLine,
  QUIET_MS,
  restartDelay,
  stopsTheLoop,
  whoOf,
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

/**
 * The golden lines, as `watch --json --since 3 --session 5e55i0n1` printed them: a ping, the
 * replay, the session's own presence, a rescan.
 */
const golden: StreamFrame[] = frames.map((f) => {
  const p = parseLine(JSON.stringify(f));
  if (p.kind !== "frame") throw new Error(`not a frame: ${JSON.stringify(f)}`);
  return p.frame;
});

test("the golden frames parse, and each moves the cursor as its kind says", () => {
  expect(golden[0]).toEqual({ type: "ping" });
  const ping = golden[0] as StreamFrame;
  expect(frameSeq(ping)).toBeNull();
  expect(frameRefreshes(ping)).toBe(false);
  // The events are the engine's, in seq order with no gap, each refreshing the pane.
  const events = golden.filter((f) => f.type === "event");
  const seqs = events.map(frameSeq);
  expect(seqs).toEqual(seqs.map((_, i) => 4 + i));
  expect(events.every(frameRefreshes)).toBe(true);
  // The rest is the one presence frame, the watch's own row, which moves no cursor and runs no
  // focus.
  const rest = golden.slice(1).filter((f) => f.type !== "event");
  expect(rest.map((f) => f.type)).toEqual(["presence"]);
  expect(frameSeq(rest[0] as StreamFrame)).toBeNull();
  expect(frameRefreshes(rest[0] as StreamFrame)).toBe(false);
  // The loop started at --since 3 ends at the last event's seq, which a restart passes on.
  expect(advanceCursor(3, golden)).toBe(seqs.at(-1));
});

test("a_presence_frame_moves_no_cursor_and_runs_no_focus", () => {
  const presence: StreamFrame = {
    type: "presence",
    actors: [
      {
        actor: { kind: "person", id: "you", name: null },
        surface: "mod",
        client_id: "mod:5e55i0n1",
        doing: null,
        focus: null,
        seen_at: "2026-01-01T00:00:00.000000Z",
      },
    ],
  };
  expect(frameSeq(presence)).toBeNull();
  expect(frameRefreshes(presence)).toBe(false);
  expect(advanceCursor(6, [presence])).toBe(6);
  expect(advanceCursor(null, [presence])).toBeNull();
});

test("presence_leaves_out_this_session_and_keeps_three", () => {
  const row = (id: string, client: string, surface: "mod" | "cli" = "mod") => ({
    actor: { kind: "person" as const, id, name: null },
    surface,
    client_id: client,
    doing: null,
    focus: null,
    seen_at: "2026-01-01T00:00:00.000000Z",
  });
  const frame = {
    type: "presence" as const,
    actors: [
      row("you", "mod:5e55i0n1"),
      row("you", "cli:41", "cli"),
      row("ada", "mod:other"),
      row("bo", "mod:third"),
      row("cy", "mod:fourth"),
    ],
  };
  // This session's own row goes, then the latest three of the rest stay, in the frame's order.
  expect(whoOf(frame, "5e55i0n1").map((r) => r.client_id)).toEqual([
    "cli:41",
    "mod:other",
    "mod:third",
  ]);
  // A different session keeps its own row and loses the one it isn't.
  expect(whoOf(frame, "other").map((r) => r.client_id)).toEqual([
    "mod:5e55i0n1",
    "cli:41",
    "mod:third",
  ]);
  expect(whoOf({ type: "presence", actors: [] }, "5e55i0n1")).toEqual([]);
});

/**
 * Test-only overlay: frames the engine can't produce yet (an agent's ghost edit and claim), laid
 * after the real golden frames. They move the cursor and refresh like any event.
 */
const agentOverlay: StreamFrame[] = [
  {
    type: "event",
    seq: 8,
    kind: "edit",
    target: "orders:repo:orders",
    project: null,
    actor: { kind: "agent", session: "5e55i0n1", role: "chat", key: null },
    status: "ghost",
    summary: "Claude proposed renaming the group",
  },
  {
    type: "event",
    seq: 9,
    kind: "claim",
    target: "worker:repo:worker",
    project: null,
    actor: { kind: "agent", session: "5e55i0n1", role: "chat", key: null },
    status: "ghost",
    summary: "Claude claimed worker for 30 min",
  },
];

test("agent ghost edits and claims (overlay) move the cursor like the engine's events", () => {
  const all = [...golden, ...agentOverlay];
  expect(all.every((f) => parseLine(JSON.stringify(f)).kind === "frame")).toBe(
    true,
  );
  expect(agentOverlay.every(frameRefreshes)).toBe(true);
  expect(advanceCursor(3, all)).toBe(9);
});

test("a reset moves the cursor back to the seq it names; pings move nothing", () => {
  expect(advanceCursor(null, [{ type: "ping" }])).toBeNull();
  expect(advanceCursor(40, [{ type: "reset", seq: 7 }])).toBe(7);
  expect(advanceCursor(40, [...golden])).toBe(40);
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

test("a_plan_only_workspaces_store_starts_the_watch", () => {
  expect(hasStore(focusPlanOnly.result as PaneModel)).toBe(true);
  expect(hasStore(focusNoRepos.result as PaneModel)).toBe(false);
});
