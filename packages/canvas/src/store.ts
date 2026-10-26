/**
 * The canvas's one store (zustand), fed by one `Transport` (DR-56): the map from
 * `map --detail graph`, the pane from `focus`, the block positions, and what the person has
 * selected or searched. Live frames refetch what they touch.
 */

import type {
  Handle,
  Level,
  MapView,
  NodeId,
  PaneModel,
  Position,
} from "@planroom/contract";
import { blocksOf, edgesOf } from "@planroom/ui";
import { createStore, type StoreApi } from "zustand/vanilla";

import {
  elkGraph,
  hasLayout,
  placed,
  positionsOf,
  type ElkGraphIn,
  type ElkGraphOut,
} from "./layout/graph";
import { handleLevel } from "./levels";
import {
  CommandError,
  TransportError,
  type Requested,
  type Transport,
} from "./transport/types";

export type Phase = "starting" | "needs_login" | "unreachable" | "ready";

/** A view the transport doesn't hold, and where its read request stands (DR-56, DR-77). */
export interface RequestView extends Requested {
  readonly command: string;
  readonly request: object;
  /** Whether this viewer can ask for it. */
  readonly canAsk: boolean;
}

export interface CanvasState {
  phase: Phase;
  /** The engine's message when it couldn't answer, if it sent one. */
  problem: string | null;
  map: MapView | null;
  pane: PaneModel | null;
  positions: Record<NodeId, Position>;
  /** Whether this load computed the layout (no stored positions), for the e2e checks. */
  laidOut: boolean;
  selected: NodeId | null;
  query: string;
  /** The block search last brought into view; a new object each time, so the map recenters. */
  revealed: { id: NodeId } | null;
  /** The level the person asked for; the top level when null. */
  level: Level | null;
  /** The view asked for when the transport doesn't hold it; null while the map shows. */
  request: RequestView | null;
  /** The transport's line about what this view can't do, such as having no live store. */
  note: string | null;
  start: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: NodeId | null) => void;
  /** Select a block and bring it into view. */
  reveal: (id: NodeId) => void;
  search: (query: string) => void;
  showLevel: (level: Level) => Promise<void>;
  run: (handle: Handle) => Promise<void>;
  /** Ask for the view the transport doesn't hold, as a read request. */
  ask: () => Promise<void>;
  /** Leave a request state for the top-level map. */
  back: () => Promise<void>;
  stop: () => void;
}

export type CanvasStore = StoreApi<CanvasState>;

/** Refetch timing for live frames (architecture.md): first frame at once, then after 100 ms of quiet, at least every 300 ms. */
const QUIET_MS = 100;
const MAX_MS = 300;

export function createCanvasStore(
  transport: Transport,
  layout: (graph: ElkGraphIn) => Promise<ElkGraphOut>,
): CanvasStore {
  let unsubscribe: (() => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastFetch = 0;
  /** Counts loads; a load that a newer one overtook drops its result. */
  let loads = 0;

  return createStore<CanvasState>()((set, get) => {
    const fail = (e: unknown) => {
      if (e instanceof TransportError && e.kind === "needs_login") {
        set({ phase: "needs_login", problem: null });
      } else if (e instanceof CommandError) {
        set({ phase: "unreachable", problem: e.message });
      } else {
        set({ phase: "unreachable", problem: null });
      }
    };

    async function load(): Promise<void> {
      // A load awaits the transport and the layout worker, and the level can change meanwhile:
      // only the newest load writes, so an older one never puts back a view the person left.
      const generation = ++loads;
      const stale = () => generation !== loads;
      const level = get().level;
      const mapRequest = { detail: "graph", level };
      const partial = transport.partial;
      const note = partial?.note() ?? null;
      // A view checks what the transport serves before it queries (DR-56).
      if (partial && !partial.serves("map", mapRequest)) {
        const pane = partial.serves("focus", {})
          ? await transport.query<PaneModel>("focus", {})
          : null;
        if (stale()) {
          return;
        }
        set({
          request: {
            ...partial.requested("map", mapRequest),
            command: "map",
            request: mapRequest,
            canAsk: partial.canAsk(),
          },
          pane,
          note,
          phase: "ready",
          problem: null,
        });
        return;
      }
      const [map, pane] = await Promise.all([
        transport.query<MapView>("map", mapRequest),
        transport.query<PaneModel>("focus", {}).catch((e: unknown) => {
          // The map stands without the pane; only a signed-out tab stops both.
          if (e instanceof TransportError && e.kind === "needs_login") {
            throw e;
          }
          return null;
        }),
      ]);
      const blocks = blocksOf(map);
      let positions = placed(blocks, {});
      let laidOut = false;
      if (map.empty === null && !hasLayout(blocks)) {
        positions = placed(
          blocks,
          positionsOf(await layout(elkGraph(blocks, edgesOf(map)))),
        );
        laidOut = true;
        if (stale()) {
          return;
        }
        // Stored under the key the engine named for this view and structure, so the next map
        // it serves carries these positions. A map with no key has nowhere to store them.
        const key = map.layout ?? null;
        if (transport.capabilities.layoutWriteBack && key !== null) {
          const write = { ...key, positions };
          // A cache: a failed write only means the next tab lays out again.
          transport.saveLayout(write).catch(() => undefined);
        }
      }
      if (stale()) {
        return;
      }
      const selected = get().selected;
      set({
        map,
        pane,
        positions,
        laidOut,
        phase: "ready",
        problem: null,
        request: null,
        note,
        selected: selected !== null && positions[selected] ? selected : null,
      });
    }

    const scheduleRefresh = () => {
      const now = Date.now();
      if (timer === null && now - lastFetch > MAX_MS) {
        lastFetch = now;
        void get().refresh();
        return;
      }
      if (timer !== null) {
        clearTimeout(timer);
      }
      const wait = Math.min(QUIET_MS, Math.max(0, lastFetch + MAX_MS - now));
      timer = setTimeout(() => {
        timer = null;
        lastFetch = Date.now();
        void get().refresh();
      }, wait);
    };

    return {
      phase: "starting",
      problem: null,
      map: null,
      pane: null,
      positions: {},
      laidOut: false,
      selected: null,
      query: "",
      revealed: null,
      level: null,
      request: null,
      note: null,
      async start() {
        try {
          if (!(await transport.login())) {
            set({ phase: "needs_login" });
            return;
          }
          lastFetch = Date.now();
          await load();
          unsubscribe ??= transport.subscribe(
            (frame) => {
              if (frame.type !== "ping") {
                scheduleRefresh();
              }
            },
            (e) => fail(e),
          );
        } catch (e) {
          fail(e);
        }
      },
      async refresh() {
        try {
          await load();
        } catch (e) {
          fail(e);
        }
      },
      select(id) {
        set({ selected: id });
      },
      reveal(id) {
        set({ selected: id, revealed: { id } });
      },
      search(query) {
        set({ query });
      },
      async showLevel(level) {
        set({ level, selected: null });
        await get().refresh();
      },
      async run(handle) {
        // A map handle, such as the pane's "Open the containers", is a level to show; one with
        // no level, such as an empty level's way back, is the top-level map.
        const level = handleLevel(handle);
        if (level !== undefined) {
          await (level === null ? get().back() : get().showLevel(level));
          return;
        }
        try {
          await transport.run(handle);
          await load();
        } catch (e) {
          fail(e);
        }
      },
      async ask() {
        const request = get().request;
        if (request === null || transport.partial === null) {
          return;
        }
        try {
          await transport.partial.ask(request.command, request.request);
          await load();
        } catch (e) {
          fail(e);
        }
      },
      async back() {
        set({ level: null, selected: null, request: null });
        await get().refresh();
      },
      stop() {
        unsubscribe?.();
        unsubscribe = null;
        if (timer !== null) {
          clearTimeout(timer);
        }
      },
    };
  });
}
