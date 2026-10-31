/**
 * A hand gesture as one `edit` leaf (DR-68). Pure: no store and no React. A gesture names what the
 * person did; `editRequest` gives the wire request the engine takes, with the `seq` of the map
 * drawn as its `base`, so an edit made on a view that has since moved reports a conflict instead
 * of applying (DR-8).
 */

import {
  CANVAS_STRINGS,
  type Handle,
  type Input,
  type MapNode,
  type MapView,
  type NodeId,
} from "@planroom/contract";
import { fill, sanitize } from "@planroom/ui";

import type { PendingEdit } from "./pending";

export type Gesture =
  | { op: "rename"; id: NodeId; name: string }
  | { op: "group"; name: string; ids: readonly NodeId[] }
  | { op: "hide"; id: NodeId }
  | { op: "pin"; id: NodeId; x: number; y: number }
  | { op: "move"; id: NodeId; parent: NodeId }
  | { op: "relate"; from: NodeId; to: NodeId; kind: string }
  | { op: "forbid"; from: NodeId; to: NodeId; kind: string };

const round2 = (n: number): number => Math.round(n * 100) / 100;

/** The wire request of a gesture on the drawn map, written at `base`. */
export function editRequest(
  g: Gesture,
  map: MapView,
  base: number,
): Input.EditReq {
  switch (g.op) {
    case "group":
      return { op: "group", name: g.name, ids: [...g.ids], base };
    case "pin":
      return {
        op: "pin",
        id: g.id,
        view: map.layout?.view_key ?? "",
        x: round2(g.x),
        y: round2(g.y),
        base,
      };
    default:
      return { ...g, base };
  }
}

/**
 * The request an edit's undo handle runs: `edit --base N undo EVENT` (the engine's own handle).
 * Any other shape is no undo of an edit, so it throws.
 */
export function undoRequest(handle: Handle): Input.EditReq {
  const [flag, base, op, event, ...rest] = handle.args;
  const n = Number(base);
  if (
    handle.command !== "edit" ||
    flag !== "--base" ||
    op !== "undo" ||
    event === undefined ||
    rest.length > 0 ||
    !Number.isInteger(n)
  ) {
    throw new Error("not an edit undo handle");
  }
  return { op: "undo", event, base: n };
}

/** The handle when it is an edit's undo the canvas can send; null for any other shape. */
export function editUndo(handle: Handle | null): Handle | null {
  if (handle === null) {
    return null;
  }
  try {
    undoRequest(handle);
    return handle;
  } catch {
    return null;
  }
}

/** The pending line of the undo of the edit whose row reads `line`. */
export const undoLine = (line: string): string =>
  fill(CANVAS_STRINGS.undo_line, { line });

const subject = (g: Gesture): NodeId => {
  switch (g.op) {
    case "group":
      return g.ids[0] ?? "";
    case "relate":
    case "forbid":
      return g.from;
    default:
      return g.id;
  }
};

/** The line "Your edits" shows while the edit is unsent: the subject block's name. */
export function pendingLine(g: Gesture, map: MapView): string {
  const id = subject(g);
  const name =
    map.nodes.find((n) => n.id === id)?.name ?? id.split(":").at(-1) ?? id;
  return fill(CANVAS_STRINGS.pending_line, { name: sanitize(name) });
}

/** The relation kinds the shown map draws, which are the kinds a person can pick to relate. */
export function relationKinds(map: MapView): string[] {
  return [...new Set(map.edges.map((e) => e.kind))].sort();
}

/** The shown blocks a block can move into: every other block that isn't below it. */
export function moveTargets(map: MapView, id: NodeId): MapNode[] {
  const parentOf = new Map(map.nodes.map((n) => [n.id, n.parent ?? null]));
  const below = (node: MapNode): boolean => {
    const seen = new Set<NodeId>();
    for (
      let at = node.parent ?? null;
      at !== null && !seen.has(at);
      at = parentOf.get(at) ?? null
    ) {
      if (at === id) {
        return true;
      }
      seen.add(at);
    }
    return false;
  };
  return map.nodes.filter((n) => n.id !== id && !below(n));
}

/**
 * What Forbid offers between two blocks: one choice per relation kind the map draws between them
 * in either direction, each in its own edge's direction.
 */
export function forbidChoices(
  map: MapView,
  a: NodeId,
  b: NodeId,
): { from: NodeId; to: NodeId; kind: string }[] {
  const byKind = new Map<string, { from: NodeId; to: NodeId; kind: string }>();
  for (const e of map.edges) {
    const between =
      (e.from === a && e.to === b) || (e.from === b && e.to === a);
    if (between && !byKind.has(e.kind)) {
      byKind.set(e.kind, { from: e.from, to: e.to, kind: e.kind });
    }
  }
  return [...byKind.values()].sort((x, y) => (x.kind < y.kind ? -1 : 1));
}

export interface PendingMarks {
  /** Blocks an unsent edit touches, which the map marks. */
  marked: Set<NodeId>;
  /** The names unsent renames give. */
  names: Map<NodeId, string>;
  /** Blocks an unsent hide takes off the map. */
  hidden: Set<NodeId>;
  /** Where unsent pins put blocks. */
  pinned: Map<NodeId, { x: number; y: number }>;
  /** Unsent relations, drawn as chalk lines. */
  chalk: { idem: string; from: NodeId; to: NodeId }[];
}

/** What the map draws for the workspace's unsent edits, oldest first so a later one wins. */
export function pendingMarks(edits: readonly PendingEdit[]): PendingMarks {
  const out: PendingMarks = {
    marked: new Set(),
    names: new Map(),
    hidden: new Set(),
    pinned: new Map(),
    chalk: [],
  };
  for (const { idem, request: r } of edits) {
    switch (r.op) {
      case "rename":
        out.names.set(r.id, r.name);
        out.marked.add(r.id);
        break;
      case "hide":
        out.hidden.add(r.id);
        out.marked.add(r.id);
        break;
      case "pin":
        out.pinned.set(r.id, { x: r.x, y: r.y });
        out.marked.add(r.id);
        break;
      case "move":
        out.marked.add(r.id);
        break;
      case "group":
        r.ids.forEach((id) => out.marked.add(id));
        break;
      case "relate":
        out.marked.add(r.from).add(r.to);
        out.chalk.push({ idem, from: r.from, to: r.to });
        break;
      case "forbid":
        out.marked.add(r.from).add(r.to);
        break;
      default:
        break;
    }
  }
  return out;
}
