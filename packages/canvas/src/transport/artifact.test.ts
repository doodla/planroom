import type {
  Answer,
  MapView,
  PaneModel,
  Snapshot,
  StreamFrame,
} from "@planroom/contract";
import { describe, expect, test } from "vitest";

import { STRINGS } from "../strings";
import {
  artifactTransport,
  viewKey,
  type ArtifactHost,
  type HostDb,
  type HostDoc,
  type HostDocRef,
} from "./artifact";

function map(level: MapView["level"], ids: string[]): MapView {
  return {
    level,
    lens: "all",
    focus: null,
    nodes: ids.map((id) => ({
      id,
      kind: "pack.kind",
      tone: null,
      name: id,
      level,
      parent: null,
      counts: {},
      position: null,
      ghost: false,
      claimed_by: null,
    })),
    edges: [],
    hidden: 0,
    empty: null,
  };
}

const pane = { focus: { seq: 5 } } as unknown as PaneModel;

const snapshot: Snapshot = {
  format: "planroom.snapshot/v1",
  workspace_id: "w1",
  seq: 5,
  published_at: "2026-10-26T00:00:00Z",
  snippets: false,
  maps: [map("system", ["a", "b"])],
  cards: [],
  who: null,
  pane,
  answers: [],
  capabilities: ["map:system:all", "pane"],
  unavailable: {
    reason: "not_in_snapshot",
    next: {
      command: "map",
      args: [],
      label: "Not in this snapshot. Ask your computer for it",
    },
  },
};

/** An in-memory `db` with the calls the transport makes: last writer wins, `onSnapshot` pushes. */
function fakeDb() {
  const docs = new Map<string, Record<string, unknown>>();
  const watchers = new Map<string, Set<(snap: HostDoc) => void>>();
  let minted = 0;
  const snap = (path: string): HostDoc => {
    const data = docs.get(path);
    return {
      id: path.split("/").at(-1)!,
      exists: data !== undefined,
      data: () => data,
    };
  };
  const push = (path: string) => {
    for (const w of watchers.get(path) ?? []) {
      w(snap(path));
    }
  };
  const ref = (path: string): HostDocRef => ({
    id: path.split("/").at(-1)!,
    set(data) {
      docs.set(path, structuredClone(data));
      push(path);
      return Promise.resolve();
    },
    update(data) {
      const held = docs.get(path);
      if (held === undefined) {
        return Promise.reject(
          Object.assign(new Error("missing"), { code: "invalid_argument" }),
        );
      }
      docs.set(path, { ...held, ...data });
      push(path);
      return Promise.resolve();
    },
    onSnapshot(next) {
      const set = watchers.get(path) ?? new Set();
      set.add(next);
      watchers.set(path, set);
      queueMicrotask(() => next(snap(path)));
      return () => set.delete(next);
    },
  });
  const db: HostDb = {
    doc: ref,
    collection: (path) => ({
      doc: (id) => ref(`${path}/${id ?? `doc${++minted}`}`),
    }),
  };
  /** A write by the session, as `ArtifactData` makes one. */
  const sessionSet = (path: string, data: object) => {
    docs.set(path, structuredClone(data) as Record<string, unknown>);
    push(path);
  };
  return { db, docs, watchers, sessionSet };
}

function host(db: HostDb | null, uid: string | null = "u_1"): ArtifactHost {
  return {
    use: ((name: string) =>
      Promise.resolve(
        name === "db" ? db : { id: () => Promise.resolve(uid) },
      )) as ArtifactHost["use"],
  };
}

function transport(h: ArtifactHost | null, published: object = snapshot) {
  const fetched: string[] = [];
  const t = artifactTransport({
    fetch: ((url: string) => {
      fetched.push(url);
      return Promise.resolve(
        new Response(JSON.stringify(published), { status: 200 }),
      );
    }) as typeof fetch,
    host: h,
    instance: () => "inst-1",
    now: () => new Date("2026-10-26T01:02:03Z"),
  });
  const frames: StreamFrame[] = [];
  t.subscribe(
    (f) => frames.push(f),
    () => undefined,
  );
  return { t, frames, fetched };
}

const container = { detail: "graph", level: "container" };

const settle = () => new Promise((r) => setTimeout(r, 0));

describe("the artifact transport", () => {
  test("it draws from the published snapshot by a relative URL", async () => {
    const { t, fetched } = transport(host(null));
    expect(await t.login()).toBe(true);
    expect(fetched).toEqual(["./data/snapshot.json"]);
    expect(
      await t.query<MapView>("map", { detail: "graph", level: null }),
    ).toEqual(snapshot.maps[0]);
    expect(await t.query<PaneModel>("focus", {})).toEqual(pane);
    expect(t.capabilities).toMatchObject({
      views: ["map:system:all", "pane"],
      layoutWriteBack: false,
    });
  });

  test("with no store it renders the snapshot alone, with a note, and can't ask", async () => {
    const { t, frames } = transport(host(null));
    await t.login();
    await settle();
    const p = t.partial!;
    expect(p.note()).toBe(STRINGS.snapshotOnly);
    expect(p.canAsk()).toBe(false);
    expect(p.serves("map", container)).toBe(false);
    expect(p.requested("map", container)).toEqual({
      state: "unavailable",
      line: "Not in this snapshot. Ask your computer for it",
      problem: null,
    });
    // The page heard the platform answer, so it redraws with the note.
    expect(frames.at(-1)).toEqual({ type: "reset", seq: 5 });
    await expect(p.ask("map", container)).rejects.toThrow();
  });

  test("a subscriber that arrives after the platform answered is caught up", async () => {
    const { t } = transport(host(null));
    await t.login();
    await settle();
    const late: StreamFrame[] = [];
    t.subscribe(
      (f) => late.push(f),
      () => undefined,
    );
    await settle();
    expect(late).toEqual([{ type: "reset", seq: 5 }]);
  });

  test("a page opened outside the platform has no host at all", async () => {
    const { t } = transport(null);
    await t.login();
    await settle();
    expect(t.partial!.note()).toBe(STRINGS.snapshotOnly);
  });

  test("asking writes one read request row under the viewer's own inbox and bumps it", async () => {
    const f = fakeDb();
    const { t } = transport(host(f.db));
    await t.login();
    await settle();
    const p = t.partial!;
    expect(p.note()).toBeNull();
    expect(p.canAsk()).toBe(true);
    await p.ask("map", container);
    const rows = [...f.docs].filter(([k]) => k.startsWith("inbox/u_1/events/"));
    expect(rows).toHaveLength(1);
    expect(rows[0]![1]).toEqual({
      format: "planroom.inbox/v1",
      instance: "inst-1",
      counter: 1,
      command: "map",
      request: container,
      base: 5,
      at: "2026-10-26T01:02:03.000Z",
    });
    expect(f.docs.get("inbox/u_1")).toEqual({
      seq: 1,
      updated_at: "2026-10-26T01:02:03.000Z",
      sample: [],
    });
    expect(p.requested("map", container).state).toBe("asked");
    // Subscribed once to the answer's document.
    const docId = rows[0]![0].split("/").at(-1)!;
    expect(f.watchers.get(`answers/${docId}`)?.size).toBe(1);
  });

  test("the answer, when the sync writes it, draws the view and ends the subscription", async () => {
    const f = fakeDb();
    const { t, frames } = transport(host(f.db));
    await t.login();
    await settle();
    await t.partial!.ask("map", container);
    const docId = [...f.docs.keys()]
      .find((k) => k.startsWith("inbox/u_1/events/"))!
      .split("/")
      .at(-1)!;
    const answered = map("container", ["c1", "c2", "c3"]);
    const answer: Answer = {
      format: "planroom.answer/v1",
      handle: {
        command: "map",
        args: ["--level", "container"],
        label: "x",
      },
      seq: 6,
      result: answered as unknown as Answer["result"],
    };
    const before = frames.length;
    f.sessionSet(`answers/${docId}`, answer);
    expect(frames.length).toBe(before + 1);
    expect(t.partial!.serves("map", container)).toBe(true);
    expect(await t.query<MapView>("map", container)).toEqual(answered);
    expect(f.watchers.get(`answers/${docId}`)?.size).toBe(0);
  });

  test("a refused read shows why", async () => {
    const f = fakeDb();
    const { t } = transport(host(f.db));
    await t.login();
    await settle();
    await t.partial!.ask("map", container);
    const docId = [...f.docs.keys()]
      .find((k) => k.startsWith("inbox/u_1/events/"))!
      .split("/")
      .at(-1)!;
    f.sessionSet(`answers/${docId}`, {
      format: "planroom.answer/v1",
      handle: { command: "map", args: [], label: "x" },
      seq: 6,
      result: null,
      error: { code: "invalid_argument", message: "No such level." },
    });
    expect(t.partial!.requested("map", container)).toEqual({
      state: "refused",
      line: STRINGS.refused,
      problem: "No such level.",
    });
  });

  test("a new live head redraws the page", async () => {
    const f = fakeDb();
    const { t, frames } = transport(host(f.db));
    await t.login();
    await settle();
    const before = frames.length;
    f.sessionSet("live/head", {
      format: "planroom.live-head/v1",
      seq: 9,
      snapshot_seq: 5,
      cards: [],
      changes: [],
      conflicts: [],
    });
    expect(frames.slice(before)).toEqual([{ type: "reset", seq: 9 }]);
  });

  test("a snapshot keeps the fields it doesn't know, and one a newer engine wrote is refused", async () => {
    const later = { ...snapshot, added_later: { kept: true } };
    const { t } = transport(host(null), later);
    expect(await t.login()).toBe(true);
    expect(await t.query<PaneModel>("focus", {})).toEqual(pane);
    const newer = { ...snapshot, format: "planroom.snapshot/v2" };
    await expect(transport(host(null), newer).t.login()).rejects.toThrow(
      STRINGS.newerDocument,
    );
    const other = { ...snapshot, format: "planroom.live-head/v1" };
    await expect(transport(host(null), other).t.login()).rejects.toThrow(
      STRINGS.snapshotMissing,
    );
  });

  test("a live head a newer engine wrote isn't read; an older-shaped one with extra fields is", async () => {
    const f = fakeDb();
    const { t, frames } = transport(host(f.db));
    await t.login();
    await settle();
    const before = frames.length;
    const head = {
      format: "planroom.live-head/v1",
      seq: 9,
      snapshot_seq: 5,
      cards: [],
      changes: [],
      conflicts: [],
    };
    f.sessionSet("live/head", {
      ...head,
      seq: 12,
      format: "planroom.live-head/v2",
    });
    expect(frames.slice(before)).toEqual([]);
    f.sessionSet("live/head", { ...head, added_later: [1] });
    expect(frames.slice(before)).toEqual([{ type: "reset", seq: 9 }]);
  });

  test("an answer in a newer format shows as refused, with why", async () => {
    const f = fakeDb();
    const { t } = transport(host(f.db));
    await t.login();
    await settle();
    await t.partial!.ask("map", container);
    const docId = [...f.docs.keys()]
      .find((k) => k.startsWith("inbox/u_1/events/"))!
      .split("/")
      .at(-1)!;
    f.sessionSet(`answers/${docId}`, {
      format: "planroom.answer/v2",
      handle: { command: "map", args: [], label: "x" },
      seq: 6,
      result: null,
    });
    expect(t.partial!.requested("map", container)).toEqual({
      state: "refused",
      line: STRINGS.refused,
      problem: STRINGS.newerDocument,
    });
    expect(t.partial!.serves("map", container)).toBe(false);
  });

  test("a signed-out viewer has the store but no inbox to write", async () => {
    const f = fakeDb();
    const { t } = transport(host(f.db, null));
    await t.login();
    await settle();
    expect(t.partial!.canAsk()).toBe(false);
    expect(t.partial!.note()).toBe(STRINGS.snapshotOnly);
  });
});

test("a view key names the map's level and lens", () => {
  expect(viewKey("map", { level: null })).toBe("map:system:all");
  expect(viewKey("map", { level: null }, "runtime")).toBe("map:system:runtime");
  expect(viewKey("map", { level: null, lens: "all" }, "runtime")).toBe(
    "map:system:all",
  );
  expect(viewKey("map", { level: "container", lens: "deploy" })).toBe(
    "map:container:deploy",
  );
  expect(viewKey("focus", {})).toBe("pane");
});
