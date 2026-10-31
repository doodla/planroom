import type {
  AskItem,
  AskResult,
  Handle,
  IntentListResult,
  MapView,
  PaneModel,
  StreamFrame,
  Warning,
} from "@planroom/contract";
import { describe, expect, test, vi } from "vitest";

import type { ElkGraphIn } from "./layout/graph";
import type { EditsEnv } from "./edits/env";
import { pendingStore } from "./edits/pending";
import { createCanvasStore } from "./store";
import {
  CommandError,
  TransportError,
  type LayoutPut,
  type Reply,
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
    layout: { view_key: "map:container:all", structure_hash: "5e1f" },
    // The code pack's ladder, as every `--detail graph` reply carries it.
    levels: ["intent", "system", "container", "component", "code"].map(
      (id) => ({
        id,
        label: id,
        one: `{count} ${id}`,
        many: `{count} ${id}s`,
        apart: id === "intent",
        pack: id === "intent" ? "kernel" : "code",
      }),
    ),
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

const warning = (message: string): Warning => ({
  code: "partial_scan",
  message,
  details: null,
});

/** The map as the engine draws a request: the top when it names no level, the level below a focus. */
function echo(request: Record<string, unknown>): MapView {
  const level =
    (request.level as string | undefined) ??
    (request.focus === undefined ? "system" : "container");
  return map({
    level,
    lens: (request.lens as string | undefined) ?? "all",
    focus: (request.focus as string | undefined) ?? null,
  });
}

function refusal(message: string, hint: string | null = null): CommandError {
  return new CommandError({
    schema_version: 1,
    ok: false,
    command: "map",
    error: { code: "not_found", message, retry: "never", hint },
    meta: { seq: 0, size: { bytes: 0, tokens_est: 0 } },
  } as never);
}

function fake(answers: {
  map?: (request: Record<string, unknown>) => MapView;
  focus?: () => PaneModel;
  login?: boolean;
  /** An answer for `ask`, `intent list` or a read of the test's own; wins over the defaults. */
  over?: (
    command: string,
    request: Record<string, unknown>,
  ) => Reply<unknown> | undefined;
  /** The warnings the next `map` reply carries. */
  mapWarnings?: () => Warning[];
  /** The warnings a press carries. */
  pressWarnings?: () => Warning[];
}) {
  const saved: LayoutPut[] = [];
  const ran: Handle[] = [];
  /** The `map` and `focus` reads: what a refetch is. */
  const queries: { command: string; request: object }[] = [];
  /** The `ask` and `intent list` reads beside them. */
  const choices: { command: string; request: object }[] = [];
  let onFrame: (f: StreamFrame) => void = () => undefined;
  const transport: Transport = {
    capabilities: { views: "all", layoutWriteBack: true, edits: false },
    workspace: () => null,
    partial: null,
    login: () => Promise.resolve(answers.login ?? true),
    query: <T>(command: string, request: object) => {
      const asked = request as Record<string, unknown>;
      const mine = answers.over?.(command, asked);
      if (command === "ask" || command === "intent list") {
        choices.push({ command, request });
      } else {
        queries.push({ command, request });
      }
      try {
        if (mine !== undefined) {
          return Promise.resolve(mine as Reply<T>);
        }
        let result: unknown;
        let warnings: Warning[] = [];
        if (command === "map") {
          result = (answers.map ?? (() => map()))(asked);
          warnings = answers.mapWarnings?.() ?? [];
        } else if (command === "ask") {
          result = { question: "", summary: "", items: [], empty: null };
        } else if (command === "intent list") {
          result = { summary: "", projects: [], items: [], empty: null };
        } else {
          result = (answers.focus ?? (() => pane))();
        }
        return Promise.resolve({
          result,
          warnings,
          next: null,
          seq: 0,
          replayed: false,
          summary: null,
        } as Reply<T>);
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    },
    write: () => Promise.reject(new Error("unused")),
    run: (h) => {
      ran.push(h);
      return Promise.resolve({
        result: null,
        warnings: answers.pressWarnings?.() ?? [],
        next: null,
        seq: 0,
        replayed: false,
        summary: null,
      });
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
    choices,
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
        view_key: "map:container:all",
        structure_hash: "5e1f",
        positions: s.positions,
      },
    ]);
    expect(f.queries[0]).toEqual({
      command: "map",
      request: { detail: "graph" },
    });
  });

  test("a map with no layout key is laid out but has nowhere to store it", async () => {
    const f = fake({ map: () => map({ layout: null }) });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState().laidOut).toBe(true);
    expect(f.saved).toEqual([]);
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

test("ux-3: a map handle with no level, an empty level's way back, shows the top-level map", async () => {
  const f = fake({});
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  await store.getState().showLevel("intent");
  expect(store.getState().level).toBe("intent");
  await store
    .getState()
    .run({ command: "map", args: [], label: "Back to the system level" });
  expect(store.getState().level).toBeNull();
  expect(f.ran).toEqual([]);
  expect(f.queries.at(-2)).toEqual({
    command: "map",
    request: { detail: "graph" },
  });
});

describe("a transport that holds only some views", () => {
  function partialFake() {
    const f = fake({});
    const asked: { command: string; request: object }[] = [];
    let held = false;
    const transport: Transport = {
      ...f.transport,
      capabilities: {
        views: ["map:container:all", "pane"],
        layoutWriteBack: false,
        edits: false,
      },
      partial: {
        serves: (command, request) =>
          command !== "map" ||
          (request as { level?: string | null }).level == null ||
          held,
        requested: () =>
          asked.length > 0
            ? { state: "asked", line: "Asked", problem: null }
            : {
                state: "unavailable",
                line: "Not in this snapshot",
                problem: null,
              },
        canAsk: () => true,
        ask: (command, request) => {
          asked.push({ command, request });
          return Promise.resolve();
        },
        note: () => null,
      },
    };
    return { ...f, transport, asked, hold: () => (held = true) };
  }

  test("a level it doesn't hold shows the request state, asks once, and goes back", async () => {
    const f = partialFake();
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    const queried = f.queries.length;
    await store.getState().showLevel("component");
    expect(store.getState().request).toMatchObject({
      state: "unavailable",
      line: "Not in this snapshot",
      command: "map",
      request: { detail: "graph", level: "component" },
      canAsk: true,
    });
    // It checked before it queried: no map query for the view it doesn't hold.
    expect(f.queries.slice(queried).map((q) => q.command)).toEqual(["focus"]);
    await store.getState().ask();
    expect(f.asked).toEqual([
      { command: "map", request: { detail: "graph", level: "component" } },
    ]);
    expect(store.getState().request?.state).toBe("asked");
    await store.getState().back();
    expect(store.getState()).toMatchObject({ request: null, level: null });
    expect(store.getState().map).not.toBeNull();
  });

  test("the view draws once the transport holds it", async () => {
    const f = partialFake();
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await store.getState().showLevel("component");
    f.hold();
    await store.getState().refresh();
    expect(store.getState().request).toBeNull();
    expect(f.queries.at(-2)).toEqual({
      command: "map",
      request: { detail: "graph", level: "component" },
    });
  });

  test("a load that started before a level change never overwrites that level's state", async () => {
    const f = partialFake();
    let release: () => void = () => undefined;
    const slow = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const layout = vi.fn(async (g: ElkGraphIn) => {
      calls += 1;
      if (calls > 1) {
        await slow;
      }
      return f.layout(g);
    });
    const store = createCanvasStore(f.transport, layout);
    await store.getState().start();
    // A live frame's refresh starts at the top level and waits on the layout worker...
    const stale = store.getState().refresh();
    await vi.waitFor(() => expect(calls).toBe(2));
    // ...while the person dives into a level the transport doesn't hold.
    await store.getState().showLevel("component");
    expect(store.getState().request?.state).toBe("unavailable");
    release();
    await stale;
    expect(store.getState()).toMatchObject({
      level: "component",
      request: { state: "unavailable", canAsk: true },
    });
  });

  test("a map handle, such as the pane's next action, shows its level", async () => {
    const f = partialFake();
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await store.getState().run({
      command: "map",
      args: ["--level", "container"],
      label: "Open the containers",
    });
    expect(f.ran).toEqual([]);
    expect(store.getState().level).toBe("container");
    expect(store.getState().request?.state).toBe("unavailable");
  });
});

const lensItem = (id: string): AskItem => ({
  id,
  name: id,
  why: "",
  handles: [{ command: "map", args: ["--lens", id], label: "" }],
});

const page = <T>(result: T, next: string | null = null): Reply<T> => ({
  result,
  warnings: [],
  next,
  seq: 0,
  replayed: false,
  summary: null,
});

test("only_event_and_reset_frames_refetch_the_views", async () => {
  const f = fake({});
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  await new Promise((r) => setTimeout(r, 350));
  const before = f.queries.length;
  // A ping, a presence frame and a frame type this canvas has never heard of change no view.
  f.frame({ type: "ping" });
  f.frame({ type: "presence", actors: [] } as unknown as StreamFrame);
  f.frame({ type: "mystery" } as unknown as StreamFrame);
  await new Promise((r) => setTimeout(r, 200));
  expect(f.queries.length).toBe(before);
  f.frame({ type: "event", seq: 3 } as unknown as StreamFrame);
  await expect.poll(() => f.queries.length).toBe(before + 2);
  await new Promise((r) => setTimeout(r, 350));
  f.frame({ type: "reset", seq: 9 });
  await expect.poll(() => f.queries.length).toBe(before + 4);
  store.getState().stop();
});

describe("a refused or unreachable view after the first one drew", () => {
  test("after_the_first_view_a_refusal_keeps_the_map_shows_the_error_and_its_hint_and_puts_the_asked_view_back", async () => {
    const f = fake({
      map: (r) => {
        if (r.level === "component") {
          throw refusal("No such block.", "planroom scan");
        }
        return echo(r);
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    const drawn = store.getState().map;
    await store.getState().showLevel("component");
    const s = store.getState();
    expect(s.phase).toBe("ready");
    expect(s.problem).toBeNull();
    expect(s.map).toBe(drawn);
    expect(s.notices).toEqual([
      { line: "No such block.", hint: "planroom scan" },
    ]);
    // The asked view is the shown one again, so a live refresh does not retry the refused one...
    expect(s).toMatchObject({ level: null, lens: null, trail: [] });
    const before = f.queries.length;
    await new Promise((r) => setTimeout(r, 350));
    f.frame({ type: "reset", seq: 2 });
    await expect.poll(() => f.queries.length).toBe(before + 2);
    expect(f.queries.at(-2)).toEqual({
      command: "map",
      request: { detail: "graph" },
    });
    // ...and does not clear the error either; the next press that works does.
    expect(store.getState().notices).toHaveLength(1);
    await store.getState().showLevel("container");
    expect(store.getState().notices).toEqual([]);
    store.getState().stop();
  });

  test("an_unreachable_engine_after_the_first_view_takes_the_page", async () => {
    let down = false;
    const f = fake({
      map: (r) => {
        if (down) {
          throw new TransportError("unreachable", "gone");
        }
        return echo(r);
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    down = true;
    await store.getState().showLevel("component");
    expect(store.getState()).toMatchObject({
      phase: "unreachable",
      problem: null,
      notices: [],
    });
  });

  test("a_failed_first_view_takes_the_page_with_the_engines_hint", async () => {
    const f = fake({
      map: () => {
        throw refusal("The store is still locked.", "planroom doctor");
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    expect(store.getState()).toMatchObject({
      phase: "unreachable",
      problem: "The store is still locked.",
      hint: "planroom doctor",
      notices: [],
    });
  });
});

describe("the view as one map handle", () => {
  const handle = (...args: string[]): Handle => ({
    command: "map",
    args,
    label: "",
  });

  test("a_map_handle_keeps_its_focus_and_lens_and_a_handle_without_a_lens_keeps_the_picked_one", async () => {
    const f = fake({ map: echo });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await store
      .getState()
      .run(handle("--level", "component", "--lens", "calls", "--focus", "a"));
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "component",
      lens: "calls",
      focus: "a",
    });
    expect(store.getState()).toMatchObject({
      level: "component",
      lens: "calls",
      trail: [{ id: "a", name: "a", level: "component" }],
    });
    // A handle that names no lens keeps the picked one, and drops the focus it does not name.
    await store.getState().run(handle("--level", "code"));
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "code",
      lens: "calls",
    });
    expect(store.getState().trail).toEqual([]);
    // No level, no focus: the top, the lens still kept.
    await store.getState().run(handle());
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "system",
      lens: "calls",
    });
    // A focus already on the trail pops to it.
    await store.getState().run(handle("--focus", "a"));
    await store.getState().run(handle("--focus", "b"));
    expect(store.getState().trail.map((t) => t.id)).toEqual(["b"]);
    // A block the map does not hold is named by the last segment of its ID.
    await store.getState().run(handle("--focus", "orders:pkg:http"));
    expect(store.getState().trail[0]?.name).toBe("http");
    expect(f.ran).toEqual([]);
  });

  test("a_lens_picked_at_the_top_names_the_top_level", async () => {
    const f = fake({ map: echo });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await store.getState().pickLens("calls");
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "system",
      lens: "calls",
    });
    expect(store.getState().level).toBeNull();
    // Below the top a pick keeps the level shown, and a way back keeps the lens.
    await store.getState().showLevel("component");
    await store.getState().pickLens("uses");
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "component",
      lens: "uses",
    });
    await store.getState().back();
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "system",
      lens: "uses",
    });
    expect(f.ran).toEqual([]);
  });

  test("a_dive_asks_for_the_level_below_and_at_an_apart_level_names_that_level", async () => {
    let counts: Record<string, number> = { container: 3 };
    const f = fake({
      map: (r) => {
        const m = echo(r);
        m.nodes.forEach((n) => (n.counts = counts));
        return m;
      },
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    // From a chain level the request names no level: the engine opens the one below.
    expect(store.getState().canDive("a")).toBe(true);
    expect(store.getState().canDive("nothing")).toBe(false);
    expect(await store.getState().dive("a")).toBe(true);
    expect(f.queries.at(-2)?.request).toEqual({ detail: "graph", focus: "a" });
    expect(store.getState().trail).toEqual([
      { id: "a", name: "a", level: null },
    ]);
    expect(store.getState().level).toBeNull();
    // A block with nothing at the level below only selects.
    counts = { container: 3 };
    const asked = f.queries.length;
    expect(await store.getState().dive("b")).toBe(false);
    expect(store.getState().canDive("b")).toBe(false);
    expect(f.queries.length).toBe(asked);
    expect(store.getState().selected).toBe("b");
    // Deeper again, the lens kept.
    counts = { component: 1 };
    await store.getState().pickLens("calls");
    expect(await store.getState().dive("b")).toBe(true);
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      lens: "calls",
      focus: "b",
    });
    expect(store.getState().trail.map((t) => t.id)).toEqual(["a", "b"]);
    // A level apart from the chain opens itself: the engine would open the top under a plan block.
    counts = { intent: 2 };
    await store.getState().showLevel("intent");
    expect(await store.getState().dive("a")).toBe(true);
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "intent",
      lens: "calls",
      focus: "a",
    });
    expect(store.getState().trail).toEqual([
      { id: "a", name: "a", level: "intent" },
    ]);
    // Popping a lid goes back to the level that entry was entered with.
    await store.getState().popTo(0);
    expect(store.getState().level).toBe("intent");
    store.getState().stop();
  });

  test("a project pick shows the project's items at the level apart", async () => {
    const f = fake({
      map: echo,
      over: (command) =>
        command === "intent list"
          ? page<IntentListResult>({
              summary: "",
              projects: [{ id: "p:project:one", name: "One" }],
              items: [],
              empty: null,
            } as unknown as IntentListResult)
          : undefined,
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await expect
      .poll(() => store.getState().projects)
      .toEqual([{ id: "p:project:one", name: "One" }]);
    await store.getState().pickProject("p:project:one");
    expect(f.queries.at(-2)?.request).toEqual({
      detail: "graph",
      level: "intent",
      focus: "p:project:one",
    });
    expect(store.getState()).toMatchObject({
      pickedProject: "p:project:one",
      trail: [{ id: "p:project:one", name: "One", level: "intent" }],
    });
  });
});

test("a_press_keeps_its_warnings_until_the_next_press_and_a_load_replaces_the_views", async () => {
  let view = [warning("view one")];
  let press = [warning("press one")];
  const f = fake({
    map: echo,
    mapWarnings: () => view,
    pressWarnings: () => press,
  });
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  expect(store.getState().notices.map((n) => n.line)).toEqual(["view one"]);
  view = [warning("view two")];
  await store.getState().run({ command: "scan", args: ["/x"], label: "" });
  expect(store.getState().notices.map((n) => n.line)).toEqual([
    "press one",
    "view two",
  ]);
  // A load that is not a press replaces the view's warnings and keeps the press's.
  view = [warning("press one"), warning("view three")];
  await store.getState().showLevel("component");
  expect(store.getState().notices.map((n) => n.line)).toEqual([
    "press one",
    "view three",
  ]);
  press = [];
  await store.getState().run({ command: "scan", args: ["/x"], label: "" });
  expect(store.getState().notices.map((n) => n.line)).toEqual([
    "press one",
    "view three",
  ]);
  view = [];
  await store.getState().refresh();
  expect(store.getState().notices).toEqual([]);
});

test("choices_follow_next_for_at_most_ten_pages", async () => {
  let n = 0;
  const f = fake({
    over: (command, request) => {
      if (command === "ask") {
        n += 1;
        // The listing never ends; the canvas stops itself.
        return page<AskResult>(
          {
            question: "",
            summary: "",
            items: [
              lensItem(`lens${n}`),
              { id: "k", name: "k", why: "", handles: [] },
            ],
            empty: null,
          },
          `cursor${n}`,
        );
      }
      void request;
      return undefined;
    },
  });
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  await expect.poll(() => f.choices.length).toBe(11);
  await new Promise((r) => setTimeout(r, 50));
  const asks = f.choices.filter((c) => c.command === "ask");
  expect(asks).toHaveLength(10);
  expect(asks[0]?.request).toEqual({});
  expect(asks[1]?.request).toEqual({ cursor: "cursor1" });
  expect(f.choices.filter((c) => c.command === "intent list")).toHaveLength(1);
  // Only the lens items are kept.
  expect(store.getState().lenses.map((i) => i.id)).toHaveLength(10);
  expect(store.getState().lenses.every((i) => i.id.startsWith("lens"))).toBe(
    true,
  );
});

test("a choice that cannot be read leaves the list as it was and shows nothing", async () => {
  let fail = false;
  const f = fake({
    map: echo,
    over: (command) => {
      if (command === "ask") {
        if (fail) {
          throw refusal("Not now.");
        }
        return page<AskResult>({
          question: "",
          summary: "",
          items: [lensItem("calls")],
          empty: null,
        });
      }
      return undefined;
    },
  });
  const store = createCanvasStore(f.transport, f.layout);
  await store.getState().start();
  await expect.poll(() => store.getState().lenses.length).toBe(1);
  fail = true;
  await store.getState().refresh();
  await new Promise((r) => setTimeout(r, 20));
  expect(store.getState().lenses.map((i) => i.id)).toEqual(["calls"]);
  expect(store.getState().notices).toEqual([]);
  expect(store.getState().phase).toBe("ready");
});

describe("the evidence sheet", () => {
  test("the_evidence_sheet_reads_ask_why_between_two_blocks", async () => {
    const f = fake({
      map: echo,
      over: (command, request) =>
        command === "ask" && request.question === "why"
          ? {
              result: {
                question: "why",
                summary: "a calls b",
                items: [],
                empty: null,
              },
              warnings: [warning("truncated")],
              next: null,
              seq: 0,
              replayed: false,
              summary: null,
            }
          : undefined,
    });
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    const opening = store.getState().openEvidence("a", "b");
    expect(store.getState().evidence).toMatchObject({
      from: "a",
      to: "b",
      state: "reading",
      result: null,
    });
    await opening;
    expect(f.choices.at(-1)).toEqual({
      command: "ask",
      request: { question: "why", args: ["a", "b"] },
    });
    expect(store.getState().evidence).toMatchObject({
      state: "ready",
      result: { summary: "a calls b" },
      warnings: [{ message: "truncated" }],
    });
    store.getState().closeEvidence();
    expect(store.getState().evidence).toBeNull();
  });

  test("an answer for ends the sheet has left is dropped, and a refusal closes the sheet with a notice", async () => {
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    let refuse = false;
    const f = fake({
      map: echo,
      over: (command, request) => {
        if (command !== "ask" || request.question !== "why") {
          return undefined;
        }
        if (refuse) {
          throw refusal("No such block.");
        }
        return {
          result: {
            question: "why",
            summary: String(request.args),
            items: [],
            empty: null,
          },
          warnings: [],
          next: null,
          seq: 0,
          replayed: false,
          summary: null,
        };
      },
    });
    const slow = {
      ...f.transport,
      query: async <T>(command: string, request: object) => {
        const r = (request as { args?: string[] }).args;
        if (command === "ask" && r?.[0] === "a") {
          await held;
        }
        return f.transport.query<T>(command, request);
      },
    } satisfies Transport;
    const store = createCanvasStore(slow, f.layout);
    await store.getState().start();
    const first = store.getState().openEvidence("a", "b");
    await store.getState().openEvidence("c", "d");
    release();
    await first;
    expect(store.getState().evidence).toMatchObject({
      from: "c",
      to: "d",
      state: "ready",
      result: { summary: "c,d" },
    });
    refuse = true;
    await store.getState().openEvidence("e", "f");
    expect(store.getState().evidence).toBeNull();
    expect(store.getState().notices.map((n) => n.line)).toEqual([
      "No such block.",
    ]);
    expect(store.getState().phase).toBe("ready");
  });
});

describe("a transport that holds only some views, part two", () => {
  function partialOnly() {
    const f = fake({ map: echo });
    const asked: string[] = [];
    const transport: Transport = {
      ...f.transport,
      partial: {
        serves: (command) => {
          asked.push(command);
          return command === "map" || command === "focus";
        },
        requested: () => ({ state: "unavailable", line: "", problem: null }),
        canAsk: () => false,
        ask: () => Promise.resolve(),
        note: () => null,
      },
    };
    return { ...f, transport, asked };
  }

  test("a_partial_transport_runs_no_focused_map", async () => {
    const f = partialOnly();
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    const map = (...args: string[]): Handle => ({
      command: "map",
      args,
      label: "",
    });
    expect(store.getState().canRun(map("--focus", "a"))).toBe(false);
    expect(store.getState().canRun(map("--level", "container"))).toBe(true);
    // A press that cannot run does nothing.
    await store.getState().run(map("--focus", "a"));
    expect(f.queries.filter((q) => q.command === "map")).toHaveLength(1);
    // A double press only selects.
    expect(store.getState().canDive("a")).toBe(false);
    expect(await store.getState().dive("a")).toBe(false);
    expect(store.getState().selected).toBe("a");
    expect(f.queries.filter((q) => q.command === "map")).toHaveLength(1);
  });

  test("the lists and the evidence are read only where the transport serves them", async () => {
    const f = partialOnly();
    const store = createCanvasStore(f.transport, f.layout);
    await store.getState().start();
    await new Promise((r) => setTimeout(r, 20));
    expect(f.choices).toEqual([]);
    expect(store.getState().canOpenEvidence("a", "b")).toBe(false);
    await store.getState().openEvidence("a", "b");
    expect(store.getState().evidence).toBeNull();
    expect(f.choices).toEqual([]);
  });
  test("a workspace no scan can map yet is not asked for its projects", async () => {
    for (const reason of [
      "not_scanned",
      "no_repos",
      "repos_unborn",
      "repos_unreadable",
    ]) {
      const f = fake({
        map: () =>
          map({
            nodes: [],
            edges: [],
            empty: {
              reason,
              next: { command: "focus", args: [], label: "Check again" },
            },
          }),
      });
      const store = createCanvasStore(f.transport, f.layout);
      await store.getState().start();
      await vi.waitFor(() =>
        expect(f.choices.map((c) => c.command)).toContain("ask"),
      );
      expect(
        f.choices.map((c) => c.command),
        reason,
      ).not.toContain("intent list");
    }
  });
});

describe("hand edits", () => {
  const memory = () => {
    const items = new Map<string, string>();
    return {
      getItem: (k: string) => items.get(k) ?? null,
      setItem: (k: string, v: string) => void items.set(k, v),
      removeItem: (k: string) => void items.delete(k),
    };
  };

  const landed = (summary: string | null = "Hid it."): Reply<unknown> => ({
    result: {
      event: "E1",
      count: 1,
      status: "applied",
      targets: ["a"],
      undo: {
        command: "edit",
        args: ["--base", "8", "undo", "E1"],
        label: "Undo",
      },
    },
    warnings: [],
    next: null,
    seq: 8,
    replayed: false,
    summary,
  });

  const stale = () =>
    new CommandError({
      schema_version: 1,
      ok: false,
      command: "edit",
      error: {
        code: "stale_write",
        message: "The map moved on.",
        retry: "after_reread",
      },
      meta: { seq: 9, size: { bytes: 0, tokens_est: 0 } },
    } as never);

  function setup(
    write: (
      request: Record<string, unknown>,
      idem: string,
      n: number,
    ) => Promise<Reply<unknown>>,
    edits = true,
  ) {
    const f = fake({
      over: (command) =>
        command === "map"
          ? {
              result: map(),
              warnings: [],
              next: null,
              seq: 7,
              replayed: false,
              summary: null,
            }
          : undefined,
    });
    const sent: { request: Record<string, unknown>; idem: string }[] = [];
    const transport: Transport = {
      ...f.transport,
      capabilities: { views: "all", layoutWriteBack: true, edits },
      workspace: () => "w1",
      write: <T>(_command: string, request: object, opts: { idem: string }) => {
        sent.push({
          request: request as Record<string, unknown>,
          idem: opts.idem,
        });
        return write(
          request as Record<string, unknown>,
          opts.idem,
          sent.length,
        ) as Promise<Reply<T>>;
      },
    };
    let n = 0;
    const env: EditsEnv = {
      local: memory(),
      session: memory(),
      uuid: () => `u${++n}`,
      now: () => 100 + n,
    };
    const store = createCanvasStore(transport, f.layout, env);
    return { store, f, sent, env, transport };
  }

  test("a_gesture_is_stored_then_sent_with_the_seq_of_the_map_drawn", async () => {
    const t = setup(() => Promise.resolve(landed()));
    await t.store.getState().start();
    expect(t.store.getState().canEdit()).toBe(true);
    const before = t.f.queries.filter((q) => q.command === "map").length;
    await t.store.getState().edit({ op: "hide", id: "a" });
    expect(t.sent).toHaveLength(1);
    expect(t.sent[0]?.request).toMatchObject({ op: "hide", id: "a", base: 7 });
    expect(String(t.sent[0]?.request.client)).toMatch(/^tab:[0-9a-f]{16}$/);
    const state = t.store.getState();
    expect(state.pending).toEqual([]);
    expect(state.edits.map((e) => [e.state, e.line])).toEqual([
      ["done", "Hid it."],
    ]);
    expect(state.edits[0]?.undo?.args).toEqual(["--base", "8", "undo", "E1"]);
    await vi.waitFor(() =>
      expect(
        t.f.queries.filter((q) => q.command === "map").length,
      ).toBeGreaterThan(before),
    );
  });

  test("an_edit_sent_while_the_engine_is_unreachable_stays_pending_until_a_ping", async () => {
    let up = false;
    const t = setup(() =>
      up
        ? Promise.resolve(landed())
        : Promise.reject(new TransportError("unreachable", "down")),
    );
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    expect(t.store.getState().pending).toHaveLength(1);
    expect(t.store.getState().edits[0]?.state).toBe("sending");
    up = true;
    t.f.frame({ type: "ping" });
    await vi.waitFor(() => expect(t.store.getState().pending).toEqual([]));
    expect(t.store.getState().edits[0]?.state).toBe("done");
    expect(t.sent[1]?.idem).toBe(t.sent[0]?.idem);
  });

  test("edits_an_earlier_page_left_are_sent_before_the_first_view_is_read", async () => {
    const t = setup(() => Promise.resolve(landed()));
    pendingStore(t.env.local, "w1").add({
      idem: "kept",
      client: "tab:0123456789abcdef",
      request: {
        op: "hide",
        id: "a",
        base: 3,
        client: "tab:0123456789abcdef",
      },
      line: "Your edit to a",
      at: 50,
    });
    await t.store.getState().start();
    expect(t.sent.map((s) => s.idem)).toEqual(["kept"]);
    expect(t.sent[0]?.request.client).toBe("tab:0123456789abcdef");
    expect(t.store.getState().pending).toEqual([]);
  });

  test("a_scan_the_canvas_runs_offers_edits_without_a_new_sign_in", async () => {
    const t = setup(() => Promise.resolve(landed()));
    // Signed in before the first scan: the tab knows no workspace until the scan's reply names it.
    let workspace: string | null = null;
    const transport: Transport = {
      ...t.f.transport,
      capabilities: { views: "all", layoutWriteBack: true, edits: true },
      workspace: () => workspace,
      run: (h) => {
        workspace = "w1";
        return t.f.transport.run(h);
      },
      write: <T>(_command: string, request: object, opts: { idem: string }) => {
        t.sent.push({
          request: request as Record<string, unknown>,
          idem: opts.idem,
        });
        return Promise.resolve(landed() as Reply<T>);
      },
    };
    const store = createCanvasStore(transport, t.f.layout, t.env);
    await store.getState().start();
    expect(store.getState().workspaceId).toBeNull();
    expect(store.getState().canEdit()).toBe(false);
    // Nothing is kept for a tab that knows no workspace.
    await store.getState().edit({ op: "hide", id: "a" });
    expect(t.sent).toEqual([]);
    expect(store.getState().pending).toEqual([]);
    await store
      .getState()
      .run({ command: "scan", args: ["/home/you/shop"], label: "Map" });
    expect(store.getState().workspaceId).toBe("w1");
    expect(store.getState().canEdit()).toBe(true);
    await store.getState().edit({ op: "hide", id: "a" });
    expect(t.sent).toHaveLength(1);
    expect(store.getState().edits[0]?.state).toBe("done");
  });

  test("a_busy_store_keeps_the_edit_sending_until_the_next_flush", async () => {
    let busy = true;
    const t = setup(() =>
      busy
        ? Promise.reject(
            new CommandError({
              schema_version: 1,
              ok: false,
              command: "edit",
              error: {
                code: "store_busy",
                message: "The store is busy.",
                retry: "now",
              },
              meta: { seq: 7, size: { bytes: 0, tokens_est: 0 } },
            } as never),
          )
        : Promise.resolve(landed()),
    );
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    expect(t.store.getState().pending).toHaveLength(1);
    expect(t.store.getState().edits[0]?.state).toBe("sending");
    busy = false;
    t.f.frame({ type: "ping" });
    await vi.waitFor(() => expect(t.store.getState().pending).toEqual([]));
    expect(t.store.getState().edits[0]?.state).toBe("done");
    expect(t.sent[1]?.idem).toBe(t.sent[0]?.idem);
  });

  test("a_held_edits_notice_leaves_when_that_edit_lands_and_no_other_notice_does", async () => {
    let busy = true;
    const t = setup(() =>
      busy
        ? Promise.reject(
            new CommandError({
              schema_version: 1,
              ok: false,
              command: "edit",
              error: {
                code: "store_busy",
                message: "The store is busy.",
                retry: "now",
              },
              meta: { seq: 7, size: { bytes: 0, tokens_est: 0 } },
            } as never),
          )
        : Promise.resolve(landed()),
    );
    const lines = () => t.store.getState().notices.map((n) => n.line);
    await t.store.getState().start();
    // The held edit's notice goes once that edit lands.
    await t.store.getState().edit({ op: "hide", id: "a" });
    expect(lines()).toEqual(["The store is busy."]);
    busy = false;
    t.f.frame({ type: "ping" });
    await vi.waitFor(() =>
      expect(t.store.getState().edits[0]?.state).toBe("done"),
    );
    await vi.waitFor(() => expect(lines()).toEqual([]));
    // A notice for something else, shown after the hold, stays when the held edit lands.
    busy = true;
    await t.store.getState().edit({ op: "hide", id: "b" });
    expect(lines()).toEqual(["The store is busy."]);
    t.transport.run = () => Promise.reject(refusal("No such block."));
    await t.store
      .getState()
      .run({ command: "scan", args: ["/home/you/shop"], label: "Map" });
    expect(lines()).toEqual(["No such block."]);
    busy = false;
    t.f.frame({ type: "ping" });
    await vi.waitFor(() => expect(t.store.getState().pending).toEqual([]));
    expect(t.store.getState().edits[0]?.state).toBe("done");
    expect(lines()).toEqual(["No such block."]);
  });

  test("a_stale_edit_shows_a_conflict_row_and_leaves_the_store", async () => {
    const t = setup(() => Promise.reject(stale()));
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    const state = t.store.getState();
    expect(state.pending).toEqual([]);
    expect(state.edits.map((e) => [e.state, e.line])).toEqual([
      ["conflict", "The map moved on."],
    ]);
  });

  test("edits_flush_one_at_a_time_in_the_order_made", async () => {
    let open = 0;
    let most = 0;
    const t = setup(async () => {
      most = Math.max(most, ++open);
      await new Promise((r) => setTimeout(r, 5));
      open--;
      return landed();
    });
    await t.store.getState().start();
    await Promise.all([
      t.store.getState().edit({ op: "hide", id: "a" }),
      t.store.getState().edit({ op: "rename", id: "b", name: "B2" }),
    ]);
    await vi.waitFor(() => expect(t.sent).toHaveLength(2));
    await vi.waitFor(() => expect(t.store.getState().pending).toEqual([]));
    expect(most).toBe(1);
    expect(t.sent.map((s) => s.request.op)).toEqual(["hide", "rename"]);
  });

  test("an_undo_is_a_new_edit_with_its_own_idem", async () => {
    const t = setup(() => Promise.resolve(landed()));
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    const row = t.store.getState().edits[0];
    expect(row).toBeDefined();
    await t.store.getState().undo(row!);
    expect(t.sent[1]?.request).toMatchObject({
      op: "undo",
      event: "E1",
      base: 8,
    });
    expect(t.sent[1]?.idem).not.toBe(t.sent[0]?.idem);
  });

  test("an_undo_row_says_what_it_undoes", async () => {
    // The undo's reply never comes, so its row keeps the canvas's own line.
    const t = setup((_r, _i, n) =>
      n === 1 ? Promise.resolve(landed()) : new Promise(() => {}),
    );
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    const row = t.store.getState().edits[0];
    expect(row?.line).toBe("Hid it.");
    void t.store.getState().undo(row!);
    await vi.waitFor(() => expect(t.store.getState().edits).toHaveLength(2));
    const [undo, edit] = t.store.getState().edits;
    expect(edit?.line).toBe("Hid it.");
    expect(undo?.line).toBe("Undo: Hid it.");
  });

  test("a_reply_whose_undo_is_no_edit_undo_offers_no_undo", async () => {
    const odd = landed();
    (odd.result as { undo: unknown }).undo = {
      command: "edit",
      args: ["undo", "E1"],
      label: "Undo",
    };
    const t = setup(() => Promise.resolve(odd));
    await t.store.getState().start();
    await t.store.getState().edit({ op: "hide", id: "a" });
    const row = t.store.getState().edits[0];
    expect(row?.state).toBe("done");
    expect(row?.undo).toBeNull();
    // Pressing it anyway sends nothing and throws nothing.
    await expect(t.store.getState().undo(row!)).resolves.toBeUndefined();
    expect(t.sent).toHaveLength(1);
  });

  test("a_transport_without_edits_cannot_edit", async () => {
    const t = setup(() => Promise.resolve(landed()), false);
    await t.store.getState().start();
    expect(t.store.getState().canEdit()).toBe(false);
    await t.store.getState().edit({ op: "hide", id: "a" });
    expect(t.sent).toEqual([]);
    expect(t.store.getState().pending).toEqual([]);
  });

  test("the_tray_closes_when_the_selection_or_the_view_changes", async () => {
    const t = setup(() => Promise.resolve(landed()));
    await t.store.getState().start();
    t.store.getState().openTray("a");
    expect(t.store.getState().trayOpen).toBe(true);
    expect(t.store.getState().selected).toBe("a");
    t.store.getState().select("a");
    expect(t.store.getState().trayOpen).toBe(true);
    t.store.getState().select("b");
    expect(t.store.getState().trayOpen).toBe(false);
    t.store.getState().openTray("a");
    await t.store.getState().showLevel("component");
    expect(t.store.getState().trayOpen).toBe(false);
  });
});
