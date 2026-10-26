import type {
  Handle,
  MapView,
  PaneModel,
  StreamFrame,
} from "@planroom/contract";
import { describe, expect, test, vi } from "vitest";

import type { ElkGraphIn } from "./layout/graph";
import { createCanvasStore } from "./store";
import {
  CommandError,
  TransportError,
  type LayoutPut,
  type Transport,
} from "./transport/types";

function map(over: Partial<MapView> = {}): MapView {
  const node = (
    id: string,
    position: MapView["nodes"][number]["position"] = null,
  ) => ({
    id,
    kind: "pack.kind",
    tone: null,
    name: id,
    level: "container" as const,
    parent: null,
    counts: {},
    position,
    ghost: false,
    claimed_by: null,
  });
  return {
    level: "container",
    lens: "all",
    focus: null,
    nodes: [node("a"), node("b")],
    edges: [
      {
        kind: "pack.k",
        from: "a",
        to: "b",
        count: 2,
        evidence_sample: [],
        children: [],
      },
    ],
    hidden: 0,
    empty: null,
    ...over,
  };
}

const pane = {
  focus: {
    breadcrumb: [],
    level: "container",
    lens: "all",
    project: null,
    seq: 1,
  },
} as unknown as PaneModel;

function fake(answers: {
  map?: () => MapView;
  focus?: () => PaneModel;
  login?: boolean;
}) {
  const saved: LayoutPut[] = [];
  const ran: Handle[] = [];
  const queries: { command: string; request: object }[] = [];
  let onFrame: (f: StreamFrame) => void = () => undefined;
  const transport: Transport = {
    capabilities: { views: "all", layoutWriteBack: true },
    login: () => Promise.resolve(answers.login ?? true),
    query: <T>(command: string, request: object) => {
      queries.push({ command, request });
      try {
        const answer =
          command === "map"
            ? (answers.map ?? (() => map()))()
            : (answers.focus ?? (() => pane))();
        return Promise.resolve(answer as T);
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    },
    write: () => Promise.reject(new Error("unused")),
    run: (h) => {
      ran.push(h);
      return Promise.resolve(null);
    },
    saveLayout: (l) => {
      saved.push(l);
      return Promise.resolve();
    },
    subscribe: (f) => {
      onFrame = f;
      return () => undefined;
    },
  };
  const layout = vi.fn((g: ElkGraphIn) =>
    Promise.resolve({
      children: g.children.map((c, i) => ({ id: c.id, x: i * 300, y: 0 })),
    }),
  );
  return {
    transport,
    layout,
    saved,
    ran,
    queries,
    frame: (f: StreamFrame) => onFrame(f),
  };
}

describe("start", () => {
  test("a tab with no link needs one", async () => {
    const f = fake({ login: false });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState().phase).toBe("needs_login");
    expect(f.queries).toHaveLength(0);
  });

  test("a map with no stored layout is laid out in the worker and written back", async () => {
    const f = fake({});
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    const s = store.getState();
    expect(s.phase).toBe("ready");
    expect(s.laidOut).toBe(true);
    expect(s.positions).toEqual({ a: { x: 0, y: 0 }, b: { x: 300, y: 0 } });
    expect(f.layout).toHaveBeenCalledOnce();
    expect(f.saved).toEqual([
      {
        view_key: "container|all|",
        structure_hash: expect.stringMatching(/^[0-9a-f]{16}$/) as string,
        positions: s.positions,
      },
    ]);
    expect(f.queries[0]).toEqual({
      command: "map",
      request: { detail: "graph", level: null },
    });
  });

  test("stored positions are used as they are, with no layout and no write", async () => {
    const f = fake({
      map: () => {
        const m = map();
        m.nodes.forEach((n, i) => (n.position = { x: i, y: i }));
        return m;
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState().positions).toEqual({
      a: { x: 0, y: 0 },
      b: { x: 1, y: 1 },
    });
    expect(f.layout).not.toHaveBeenCalled();
    expect(f.saved).toHaveLength(0);
  });

  test("an empty map lays nothing out", async () => {
    const empty = map({
      nodes: [],
      edges: [],
      empty: {
        reason: "not_scanned",
        next: {
          command: "scan",
          args: ["/x"],
          label: "Map these 5 repos? About 2 s.",
        },
      },
    });
    const f = fake({ map: () => empty });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState().map?.empty?.next.label).toBe(
      "Map these 5 repos? About 2 s.",
    );
    expect(f.layout).not.toHaveBeenCalled();
  });

  test("the map stands when the pane can't be read", async () => {
    const f = fake({
      focus: () => {
        throw new TransportError("unreachable", "down");
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState()).toMatchObject({ phase: "ready", pane: null });
  });

  test("a refused token anywhere signs the tab out", async () => {
    const f = fake({
      focus: () => {
        throw new TransportError("needs_login", "refused");
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState().phase).toBe("needs_login");
  });

  test("a failed command shows the engine's own message", async () => {
    const f = fake({
      map: () => {
        throw new CommandError({
          schema_version: 1,
          ok: false,
          command: "map",
          error: {
            code: "store_busy",
            message: "The store is still locked.",
            retry: "now",
          },
          meta: { seq: 0, size: { bytes: 0, tokens_est: 0 } },
        } as never);
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState()).toMatchObject({
      phase: "unreachable",
      problem: "The store is still locked.",
    });
  });
});

test("a live event refetches the map and the pane", async () => {
  const f = fake({});
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  const before = f.queries.length;
  await new Promise((r) => setTimeout(r, 350));
  f.frame({ type: "ping" });
  expect(f.queries.length).toBe(before);
  f.frame({ type: "reset", seq: 9 });
  await expect.poll(() => f.queries.length).toBe(before + 2);
  store.getState().stop();
});

test("a level from the rail asks the map for it, and a handle runs then refetches", async () => {
  const f = fake({});
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  await store.getState().showLevel("component");
  expect(f.queries.at(-2)).toEqual({
    command: "map",
    request: { detail: "graph", level: "component" },
  });
  const handle = { command: "scan", args: ["/x"], label: "Scan" };
  await store.getState().run(handle);
  expect(f.ran).toEqual([handle]);
  expect(f.queries.at(-2)?.command).toBe("map");
});
