/**
 * The canvas's one store (zustand), fed by one `Transport` (DR-56): the map from
 * `map --detail graph`, the pane from `focus`, the block positions, and what the person has
 * selected or searched. Live frames refetch what they touch.
 *
 * The view the person asks for is always the arguments of one `map` handle (DR-119): a level, a
 * lens and a trail of blocks whose last is the focus. Every way to a view (the rail, a lid, a
 * dive, a lens or project pick, an engine handle) sets those three and loads. The engine's
 * refusals and warnings stay beside the drawn map instead of replacing it.
 */

import type {
  AskItem,
  AskResult,
  Handle,
  IntentListResult,
  Level,
  MapView,
  NodeId,
  PaneModel,
  Position,
  Warning,
} from "@planroom/contract";
import { blocksOf, edgesOf, sanitize } from "@planroom/ui";
import { createStore, type StoreApi } from "zustand/vanilla";

import {
  elkGraph,
  hasLayout,
  placed,
  positionsOf,
  type ElkGraphIn,
  type ElkGraphOut,
} from "./layout/graph";
import { isLensItem } from "./lens-switcher";
import { browserEditsEnv, type EditsEnv } from "./edits/env";
import { flush } from "./edits/flush";
import {
  editRequest,
  pendingLine,
  editUndo,
  undoLine,
  undoRequest,
  type Gesture,
} from "./edits/ops";
import { pendingStore, type PendingEdit } from "./edits/pending";
import { tabId } from "./edits/tab";
import { atTop, diveOf, mapRequest, topOf, viewOf } from "./levels";
import { noticesOf, type Notice } from "./notices";
import { canRun } from "./transport/request";
import { beforeAScan } from "./views/engine-empty";
import {
  CommandError,
  TransportError,
  type Reply,
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

/** One block the person went into: where the trail's lids come from. */
export interface TrailEntry {
  readonly id: NodeId;
  /** The block's name, sanitized. */
  readonly name: string;
  /** The level asked when it was entered; null lets the engine open the level below. */
  readonly level: Level | null;
}

/** The evidence sheet: what `ask why` says between two blocks. */
export interface Evidence {
  readonly from: NodeId;
  readonly to: NodeId;
  readonly state: "reading" | "ready";
  readonly result: AskResult | null;
  readonly warnings: readonly Warning[];
}

/** One of this tab's edits as "Your edits" lists it. */
export interface EditEntry {
  readonly idem: string;
  /** The line shown: the unsent edit's, then the engine's summary or the conflict's message. */
  readonly line: string;
  readonly state: "sending" | "done" | "conflict";
  /** What undoes the edit once the engine has written it. */
  readonly undo: Handle | null;
  readonly at: number;
}

/** How many of this tab's edits "Your edits" keeps. */
const EDIT_ROWS = 10;

export interface CanvasState {
  phase: Phase;
  /** The engine's message when it couldn't answer, if it sent one. */
  problem: string | null;
  /** The command that clears `problem`, when the engine named one; sanitized. */
  hint: string | null;
  map: MapView | null;
  /** The `seq` the drawn map was read at: the `base` of an edit made on it (DR-8). */
  mapSeq: number | null;
  /** The workspace the tab is signed in to; null before a store exists. */
  workspaceId: string | null;
  /** The workspace's unsent edits, oldest first. */
  pending: readonly PendingEdit[];
  /** This tab's edits, newest first. */
  edits: readonly EditEntry[];
  /** Whether the action tray is open on the selected block. */
  trayOpen: boolean;
  pane: PaneModel | null;
  positions: Record<NodeId, Position>;
  /** Whether this load computed the layout (no stored positions), for the e2e checks. */
  laidOut: boolean;
  selected: NodeId | null;
  query: string;
  /** The block search last brought into view; a new object each time, so the map recenters. */
  revealed: { id: NodeId } | null;
  /**
   * The level the person asked for. Null is the top level or, under a focus, the level the
   * engine opens below it. What is drawn is `map.level`.
   */
  level: Level | null;
  /** The lens the person picked; null is the engine's default. It stays until another pick. */
  lens: string | null;
  /** The blocks the person went into, outermost first; the focus is the last. */
  trail: readonly TrailEntry[];
  /** What the engine said that the person should see: an error, a press's warnings, the view's. */
  notices: readonly Notice[];
  /** The `ask` listing's lenses, as the lens switcher lists them. */
  lenses: readonly AskItem[];
  /** `intent list`'s projects, as the shelf lists them. */
  projects: readonly { id: NodeId; name: string }[];
  /** The last project picked from the shelf. */
  pickedProject: NodeId | null;
  evidence: Evidence | null;
  /** The view asked for when the transport doesn't hold it; null while the map shows. */
  request: RequestView | null;
  /** The transport's line about what this view can't do, such as having no live store. */
  note: string | null;
  start: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: NodeId | null) => void;
  /** Whether a hand edit may be made now: the transport writes, a store exists and a map is drawn. */
  canEdit: () => boolean;
  /** Make one hand edit: keep it pending, then send it (DR-8). */
  edit: (gesture: Gesture) => Promise<void>;
  /** Undo a landed edit of this tab, as a new edit. */
  undo: (entry: EditEntry) => Promise<void>;
  /** Select a block and open the action tray on it. */
  openTray: (id: NodeId) => void;
  closeTray: () => void;
  /** Select a block and bring it into view. */
  reveal: (id: NodeId) => void;
  search: (query: string) => void;
  showLevel: (level: Level) => Promise<void>;
  /** Go back up to the lid at `index` of the trail. */
  popTo: (index: number) => Promise<void>;
  /** Go into a block; false (and only a selection) when it has nothing at the level below. */
  dive: (id: NodeId) => Promise<boolean>;
  /** Whether a dive into the block would go anywhere: the transport runs it and it holds blocks below. */
  canDive: (id: NodeId) => boolean;
  pickLens: (id: string) => Promise<void>;
  pickProject: (id: NodeId) => Promise<void>;
  run: (handle: Handle) => Promise<void>;
  /** Whether this transport can run `handle`; a button for one it can't isn't drawn. */
  canRun: (handle: Handle) => boolean;
  openEvidence: (from: NodeId, to: NodeId) => Promise<void>;
  closeEvidence: () => void;
  canOpenEvidence: (from: NodeId, to: NodeId) => boolean;
  /** Ask for the view the transport doesn't hold, as a read request. */
  ask: () => Promise<void>;
  /** Leave a request state, or a level or focus, for the top-level map. */
  back: () => Promise<void>;
  stop: () => void;
}

export type CanvasStore = StoreApi<CanvasState>;

/** Refetch timing for live frames (architecture.md): first frame at once, then after 100 ms of quiet, at least every 300 ms. */
const QUIET_MS = 100;
const MAX_MS = 300;

/** How many pages of a listing the canvas follows: a bound on a loop over the engine's replies. */
const CHOICE_PAGES = 10;

/**
 * The code pack's evidence question (architecture.md A14); the engine finds it by its shape
 * (app/ask.rs `why_question`).
 */
const WHY = "why";

/** The view as asked or as last drawn: the arguments of one `map` handle. */
interface Asked {
  readonly level: Level | null;
  readonly lens: string | null;
  readonly trail: readonly TrailEntry[];
}

export function createCanvasStore(
  transport: Transport,
  layout: (graph: ElkGraphIn) => Promise<ElkGraphOut>,
  edits: EditsEnv = browserEditsEnv(),
): CanvasStore {
  let unsubscribe: (() => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastFetch = 0;
  /** Counts loads; a load that a newer one overtook drops its result. */
  let loads = 0;
  /** Counts choice reads; only the newest sets the lists. */
  let choiceReads = 0;
  /** The asked view of the last successful load. */
  let shown: Asked = { level: null, lens: null, trail: [] };
  /** The parts of `notices`, in the engine's order: error, press, view. */
  let errorNotice: Notice | null = null;
  /** The notice a held edit set, and that edit: it leaves when the edit is no longer pending. */
  let heldNotice: { idem: string; notice: Notice } | null = null;
  let pressWarnings: readonly Warning[] = [];
  let viewWarnings: readonly Warning[] = [];
  /** The tail of the tab's flushes: a flush starts only after the one before it ended. */
  let flushing: Promise<void> = Promise.resolve();

  return createStore<CanvasState>()((set, get) => {
    const derived = () => noticesOf(errorNotice, pressWarnings, viewWarnings);

    /** Take the page: the engine can't be asked at all, or nothing has drawn yet. */
    const fail = (e: unknown) => {
      if (e instanceof TransportError && e.kind === "needs_login") {
        set({ phase: "needs_login", problem: null, hint: null });
      } else if (e instanceof CommandError) {
        const hint = e.envelope.error?.hint;
        set({
          phase: "unreachable",
          problem: e.message,
          hint: hint ? sanitize(hint) : null,
        });
      } else {
        set({ phase: "unreachable", problem: null, hint: null });
      }
    };

    /**
     * The level a dive into `id` opens, or null when it opens nothing. The Artifact holds the top
     * map alone: there a double press only selects.
     */
    const diveTarget = (id: NodeId): Level | null => {
      const { map } = get();
      const node = map?.nodes.find((n) => n.id === id);
      return map == null || node === undefined || transport.partial !== null
        ? null
        : diveOf(map.levels ?? [], map.level, node);
    };

    /**
     * What a failure does. Before the first view has drawn it takes the page. After, a refusal
     * keeps the drawn map and says so in place, with the asked view put back to the shown one so
     * no live refresh retries it; a transport failure still takes the page, since nothing on it
     * can work.
     */
    const refused = (e: unknown, source: "load" | "press" | "evidence") => {
      const drawn = get().map !== null || get().request !== null;
      if (!(e instanceof CommandError) || !drawn) {
        fail(e);
        return;
      }
      errorNotice = {
        line: e.message,
        hint: e.envelope.error?.hint ?? null,
      };
      set({
        ...(source === "evidence"
          ? { evidence: null }
          : { level: shown.level, lens: shown.lens, trail: shown.trail }),
        notices: derived(),
      });
    };

    /** Read every page of a listing, or null when it can't be read (the list then stays). */
    async function pages<T>(command: string): Promise<T[] | null> {
      const partial = transport.partial;
      const out: T[] = [];
      let request: object = {};
      try {
        for (let i = 0; i < CHOICE_PAGES; i++) {
          if (partial !== null && !partial.serves(command, request)) {
            return null;
          }
          const reply: Reply<T> = await transport.query<T>(command, request);
          if (reply.result === null || typeof reply.result !== "object") {
            return null;
          }
          out.push(reply.result);
          if (reply.next === null) {
            break;
          }
          request = { cursor: reply.next };
        }
      } catch {
        return null;
      }
      return out;
    }

    /** The lens and project lists, read beside a load and never awaited by it. */
    async function loadChoices(scanned: boolean): Promise<void> {
      const read = ++choiceReads;
      // Nothing is planned in a workspace no scan has made, and its store answers `intent list`
      // with an error.
      const [asked, intents] = await Promise.all([
        pages<AskResult>("ask"),
        scanned ? pages<IntentListResult>("intent list") : null,
      ]);
      if (read !== choiceReads) {
        return;
      }
      if (asked !== null) {
        set({ lenses: asked.flatMap((r) => r.items).filter(isLensItem) });
      }
      if (intents !== null) {
        set({
          projects: intents
            .flatMap((r) => r.projects)
            .map((p) => ({ id: p.id, name: p.name })),
        });
      }
    }

    async function loadMap(
      generation: number,
      keepError: boolean,
    ): Promise<void> {
      // A load awaits the transport and the layout worker, and the view can change meanwhile:
      // only the newest load writes, so an older one never puts back a view the person left.
      const stale = () => generation !== loads;
      const { level, lens, trail } = get();
      const asked: Asked = { level, lens, trail };
      const mapAsk = mapRequest(
        asked,
        topOf(get().map?.levels ?? [])?.id ?? null,
      );
      const partial = transport.partial;
      const note = partial?.note() ?? null;
      // A view checks what the transport serves before it queries (DR-56).
      if (partial && !partial.serves("map", mapAsk)) {
        const pane = partial.serves("focus", {})
          ? (await transport.query<PaneModel>("focus", {})).result
          : null;
        if (stale()) {
          return;
        }
        set({
          request: {
            ...partial.requested("map", mapAsk),
            command: "map",
            request: mapAsk,
            canAsk: partial.canAsk(),
          },
          pane,
          note,
          phase: "ready",
          problem: null,
          hint: null,
        });
        return;
      }
      const [mapReply, paneReply] = await Promise.all([
        transport.query<MapView>("map", mapAsk),
        transport.query<PaneModel>("focus", {}).catch((e: unknown) => {
          // The map stands without the pane; only a signed-out tab stops both.
          if (e instanceof TransportError && e.kind === "needs_login") {
            throw e;
          }
          return null;
        }),
      ]);
      const map = mapReply.result;
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
      shown = asked;
      // The view's own warnings are `map`'s and then `focus`'s, replaced on each load. A
      // refresh the stream started leaves the error on screen until a person's press clears it,
      // or, for a held edit's notice, until that edit lands or is dropped.
      viewWarnings = [...mapReply.warnings, ...(paneReply?.warnings ?? [])];
      if (!keepError) {
        errorNotice = null;
      }
      const selected = get().selected;
      const kept = selected !== null && positions[selected] ? selected : null;
      set({
        map,
        mapSeq: mapReply.seq,
        pane: paneReply?.result ?? null,
        positions,
        laidOut,
        phase: "ready",
        problem: null,
        hint: null,
        request: null,
        note,
        notices: derived(),
        selected: kept,
        trayOpen: kept === null ? false : get().trayOpen,
      });
      void loadChoices(!beforeAScan(map.empty));
    }

    async function load(keepError = false): Promise<void> {
      const generation = ++loads;
      try {
        await loadMap(generation, keepError);
      } catch (e) {
        // A load a newer one overtook drops its error as it drops its result.
        if (generation === loads) {
          refused(e, "load");
        }
      }
    }

    const scheduleRefresh = () => {
      const now = Date.now();
      if (timer === null && now - lastFetch > MAX_MS) {
        lastFetch = now;
        void load(true);
        return;
      }
      if (timer !== null) {
        clearTimeout(timer);
      }
      const wait = Math.min(QUIET_MS, Math.max(0, lastFetch + MAX_MS - now));
      timer = setTimeout(() => {
        timer = null;
        lastFetch = Date.now();
        void load(true);
      }, wait);
    };

    /** Ask for a view: set it, drop the selection and load. */
    const show = (view: Partial<Asked>) => {
      set({ ...view, selected: null, trayOpen: false });
      return get().refresh();
    };

    const topId = (): Level | null =>
      topOf(get().map?.levels ?? [])?.id ?? null;

    /** The pending store of `workspace`, which is the signed-in one unless `start` names it first. */
    const store = (
      workspace: string | null = get().workspaceId,
    ): ReturnType<typeof pendingStore> | null => {
      return workspace === null ? null : pendingStore(edits.local, workspace);
    };

    const setRow = (idem: string, row: Partial<EditEntry>) =>
      set({
        edits: get().edits.map((e) => (e.idem === idem ? { ...e, ...row } : e)),
      });

    /** One flush of the workspace's pending edits, and what it did to this tab's rows. */
    const flushOnce = async (): Promise<void> => {
      const pending = store();
      if (pending === null) {
        return;
      }
      const done = await flush(transport, pending);
      for (const o of done.outcomes) {
        const row = get().edits.find((e) => e.idem === o.idem);
        if (o.state === "landed") {
          if (row !== undefined) {
            setRow(o.idem, {
              state: "done",
              line: sanitize(o.summary ?? row.line),
              undo: editUndo(o.result.undo),
            });
          }
        } else if (o.state === "conflict") {
          setRow(o.idem, { state: "conflict", line: sanitize(o.message) });
        } else if (o.state === "held") {
          // The edit stays pending and its row sending; the next flush sends it again.
          errorNotice = { line: o.message, hint: o.hint };
          heldNotice = { idem: o.idem, notice: errorNotice };
          set({ notices: derived() });
        } else {
          set({ edits: get().edits.filter((e) => e.idem !== o.idem) });
          errorNotice = { line: o.message, hint: o.hint };
          set({ notices: derived() });
        }
      }
      set({ pending: pending.list() });
      // A held edit's notice belongs to that edit: once it landed or was dropped, the notice goes,
      // unless something else has put its own notice in its place since.
      if (
        heldNotice !== null &&
        !pending.list().some((e) => e.idem === heldNotice?.idem)
      ) {
        if (errorNotice === heldNotice.notice) {
          errorNotice = null;
          set({ notices: derived() });
        }
        heldNotice = null;
      }
      if (done.stopped?.kind === "needs_login") {
        fail(done.stopped);
      } else if (done.outcomes.some((o) => o.state !== "held")) {
        await load(true);
      }
    };

    /** Flush, after any flush already running in this tab; an error never blocks the next one. */
    const runFlush = (): Promise<void> => {
      const next = flushing.then(flushOnce, flushOnce);
      flushing = next.catch(() => undefined);
      return next;
    };

    /** Keep an edit pending, list it as sending and flush. */
    const send = async (
      request: PendingEdit["request"],
      line: string,
    ): Promise<void> => {
      const pending = store();
      if (pending === null) {
        return;
      }
      const client = tabId(edits.session);
      const idem = edits.uuid();
      const at = edits.now();
      pending.add({ idem, client, request: { ...request, client }, line, at });
      set({
        pending: pending.list(),
        edits: [
          { idem, line, state: "sending" as const, undo: null, at },
          ...get().edits,
        ].slice(0, EDIT_ROWS),
      });
      await runFlush();
    };

    return {
      phase: "starting",
      problem: null,
      hint: null,
      map: null,
      mapSeq: null,
      workspaceId: null,
      pending: [],
      edits: [],
      trayOpen: false,
      pane: null,
      positions: {},
      laidOut: false,
      selected: null,
      query: "",
      revealed: null,
      level: null,
      lens: null,
      trail: [],
      notices: [],
      lenses: [],
      projects: [],
      pickedProject: null,
      evidence: null,
      request: null,
      note: null,
      async start() {
        try {
          if (!(await transport.login())) {
            set({ phase: "needs_login" });
            return;
          }
          const workspaceId = transport.workspace();
          set({ workspaceId, pending: store(workspaceId)?.list() ?? [] });
          // Edits a closed tab or a stopped engine left are sent before the first view is read.
          if (get().pending.length > 0) {
            await runFlush();
          }
          lastFetch = Date.now();
          await load();
          unsubscribe ??= transport.subscribe(
            (frame) => {
              // Only a change to the store or a gap in the stream says a view may be stale; a
              // ping or who else is here says nothing about it.
              if (frame.type === "event" || frame.type === "reset") {
                scheduleRefresh();
              }
              // Each connection opens with a ping and one comes every 15 s: a reconnect, or an
              // edit the engine couldn't be reached for, flushes here.
              if (frame.type === "ping" && get().pending.length > 0) {
                runFlush().catch(() => undefined);
              }
            },
            (e) => fail(e),
          );
        } catch (e) {
          fail(e);
        }
      },
      async refresh() {
        await load();
      },
      select(id) {
        set({
          selected: id,
          trayOpen: id !== null && id === get().selected && get().trayOpen,
        });
      },
      canEdit() {
        const { workspaceId, map } = get();
        return (
          transport.capabilities.edits &&
          workspaceId !== null &&
          map?.layout != null &&
          map.empty == null
        );
      },
      async edit(gesture) {
        const { map, mapSeq } = get();
        if (!get().canEdit() || map === null || mapSeq === null) {
          return;
        }
        await send(
          editRequest(gesture, map, mapSeq),
          pendingLine(gesture, map),
        );
      },
      async undo(entry) {
        const row = get().edits.find((e) => e.idem === entry.idem);
        const handle = editUndo(row?.undo ?? null);
        if (row === undefined || row.state !== "done" || handle === null) {
          return;
        }
        await send(undoRequest(handle), undoLine(row.line));
      },
      openTray(id) {
        set({ selected: id, trayOpen: true });
      },
      closeTray() {
        set({ trayOpen: false });
      },
      reveal(id) {
        set({ selected: id, revealed: { id } });
      },
      search(query) {
        set({ query });
      },
      async showLevel(level) {
        if (level === topId()) {
          await get().back();
          return;
        }
        await show({ level, trail: [] });
      },
      async popTo(index) {
        const trail = get().trail.slice(0, index + 1);
        const entry = trail[index];
        if (entry === undefined) {
          return;
        }
        await show({ level: entry.level, trail });
      },
      canDive(id) {
        return diveTarget(id) !== null;
      },
      async dive(id) {
        const { map } = get();
        const node = map?.nodes.find((n) => n.id === id);
        const next = diveTarget(id);
        if (map === null || node === undefined || next === null) {
          get().select(id);
          return false;
        }
        // From a chain level the request names no level and the engine opens the one below; a
        // level apart is named, since the engine would open the top under one of its blocks.
        const apart =
          (map.levels ?? []).find((l) => l.id === map.level)?.apart ?? false;
        const level = apart ? next : null;
        await show({
          level,
          trail: [...get().trail, { id, name: sanitize(node.name), level }],
        });
        return true;
      },
      async pickLens(id) {
        const { map, trail, level } = get();
        if (map === null) {
          return;
        }
        // A lens is which relationships you see, not where you are: the level and focus stay.
        // (The listing's own handle, `map --lens ID` alone, would jump to the lens's level.)
        const asked = atTop(map.level, map.focus, topId() ?? "")
          ? null
          : trail.length > 0
            ? level
            : map.level;
        await show({ lens: id, level: asked });
      },
      async pickProject(id) {
        const { map, projects } = get();
        const project = projects.find((p) => p.id === id);
        const apart = map?.levels?.find((l) => l.apart)?.id;
        if (project === undefined || apart === undefined) {
          return;
        }
        set({ pickedProject: id });
        await show({
          level: apart,
          trail: [{ id, name: sanitize(project.name), level: apart }],
        });
      },
      async run(handle) {
        if (!get().canRun(handle)) {
          return;
        }
        // A map handle, such as the pane's "Open the containers" or a dive's `--focus`, is a
        // view: its level, lens and focus become the asked view together.
        const { map, trail: current } = get();
        const view = viewOf(map?.levels ?? [], handle);
        if (view !== undefined) {
          const level = view.level ?? null;
          const lens = view.lens ?? get().lens;
          const focus = view.focus;
          let trail: readonly TrailEntry[] = [];
          if (focus !== undefined) {
            const at = current.findIndex((t) => t.id === focus);
            const name = map?.nodes.find((n) => n.id === focus)?.name;
            trail =
              at >= 0
                ? current.slice(0, at + 1)
                : [
                    {
                      id: focus,
                      name: sanitize(name ?? focus.split(":").at(-1) ?? focus),
                      level,
                    },
                  ];
          }
          // At the top the result is `back()`: the level the top names is no level asked.
          const top = topId();
          const asked =
            top !== null && atTop(level, focus ?? null, top) ? null : level;
          await show({ level: asked, lens, trail });
          return;
        }
        try {
          const reply = await transport.run(handle);
          pressWarnings = reply.warnings;
          errorNotice = null;
          set({ notices: derived() });
          // A press can make the store (the first scan): its workspace keys this tab's edits.
          const workspaceId = transport.workspace();
          if (workspaceId !== get().workspaceId) {
            set({ workspaceId, pending: store(workspaceId)?.list() ?? [] });
          }
        } catch (e) {
          refused(e, "press");
          return;
        }
        await load();
      },
      canRun(handle) {
        return canRun(transport.partial, handle);
      },
      async openEvidence(from, to) {
        if (!get().canOpenEvidence(from, to)) {
          return;
        }
        const request = { question: WHY, args: [from, to] };
        set({
          evidence: { from, to, state: "reading", result: null, warnings: [] },
        });
        const current = () => {
          const e = get().evidence;
          return e !== null && e.from === from && e.to === to;
        };
        try {
          const reply = await transport.query<AskResult>("ask", request);
          if (current()) {
            set({
              evidence: {
                from,
                to,
                state: "ready",
                result: reply.result,
                warnings: reply.warnings,
              },
            });
          }
        } catch (e) {
          // The sheet moved on to other blocks, or closed: its answer, or its refusal, is moot.
          if (current() || !(e instanceof CommandError)) {
            refused(e, "evidence");
          }
        }
      },
      closeEvidence() {
        set({ evidence: null });
      },
      canOpenEvidence(from, to) {
        const partial = transport.partial;
        return (
          partial === null ||
          partial.serves("ask", { question: WHY, args: [from, to] })
        );
      },
      async ask() {
        const request = get().request;
        if (request === null || transport.partial === null) {
          return;
        }
        try {
          await transport.partial.ask(request.command, request.request);
        } catch (e) {
          refused(e, "press");
          return;
        }
        await load();
      },
      async back() {
        set({ request: null });
        await show({ level: null, trail: [] });
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
