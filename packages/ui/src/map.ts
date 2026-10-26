/**
 * `MapView` as surface-neutral rows: what the canvas draws as blocks and lifted edges and the
 * screen-reader outline reads. Every label is sanitized; nothing here names a kind or a lens.
 */

import type {
  Level,
  MapView,
  NodeId,
  Position,
  Tone,
} from "@planroom/contract";

import { sanitize } from "./display";

/** The levels from the plan down to the code, in the contract's order. */
export const LEVELS: readonly Level[] = [
  "intent",
  "system",
  "container",
  "component",
  "code",
];

/** One block on the map. */
export interface BlockRow {
  readonly id: NodeId;
  readonly name: string;
  /** The kind without its pack, as words: `code.queue_publish` reads `queue publish`. */
  readonly kindLabel: string;
  readonly tone: Tone | null;
  readonly level: Level;
  /** Descendants by level, top level first; a block that holds others shows a lid. */
  readonly counts: readonly {
    // A level name, as `MapNode.counts` keys it.
    readonly level: string;
    readonly count: number;
  }[];
  readonly holds: boolean;
  readonly position: Position | null;
  readonly ghost: boolean;
}

/** One lifted edge between two blocks on the map. */
export interface EdgeRow {
  readonly id: string;
  readonly from: NodeId;
  readonly to: NodeId;
  readonly kindLabel: string;
  readonly count: number;
}

/** `code.queue_publish` as `queue publish`. */
export function kindLabel(kind: string): string {
  const short = kind.includes(".") ? kind.slice(kind.indexOf(".") + 1) : kind;
  return sanitize(short.replaceAll("_", " "));
}

function levelRank(level: string): number {
  const i = LEVELS.indexOf(level as Level);
  return i < 0 ? LEVELS.length : i;
}

/** The view's blocks, in the engine's order. */
export function blockRows(view: MapView): BlockRow[] {
  return view.nodes.map((node) => {
    const counts = Object.entries(node.counts)
      .filter(([, count]) => count > 0)
      .sort(([a], [b]) => levelRank(a) - levelRank(b) || a.localeCompare(b))
      .map(([level, count]) => ({ level, count }));
    return {
      id: node.id,
      name: sanitize(node.name),
      kindLabel: kindLabel(node.kind),
      tone: node.tone ?? null,
      level: node.level,
      counts,
      holds: counts.length > 0,
      position: node.position,
      ghost: node.ghost,
    };
  });
}

/** The view's lifted edges whose two ends are both on the map, one row per kind and pair. */
export function edgeRows(view: MapView): EdgeRow[] {
  const ids = new Set(view.nodes.map((n) => n.id));
  return view.edges
    .filter((e) => ids.has(e.from) && ids.has(e.to))
    .map((e) => ({
      id: `${e.from}>${e.to}>${e.kind}`,
      from: e.from,
      to: e.to,
      kindLabel: kindLabel(e.kind),
      count: e.count,
    }));
}

/** The key a stored layout is filed under: the level, lens and focus it lays out. */
export function viewKey(view: MapView): string {
  return `${view.level}|${view.lens}|${view.focus ?? ""}`;
}

/** 32-bit FNV-1a over `text`, as 8 hex digits. */
function fnv1a(text: string, seed: number): string {
  let hash = seed >>> 0;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

/**
 * A hash of what a layout depends on, the blocks and the pairs they connect, so a stored layout
 * is reused only for the same structure. Names, counts and positions don't count.
 */
export function structureHash(view: MapView): string {
  const nodes = view.nodes.map((n) => `${n.id}<${n.parent ?? ""}`).sort();
  const edges = [...new Set(view.edges.map((e) => `${e.from}>${e.to}`))].sort();
  const text = `${nodes.join("\n")}\n--\n${edges.join("\n")}`;
  return fnv1a(text, 0x811c9dc5) + fnv1a(text, 0x01234567);
}

/** The IDs of the blocks whose name or kind holds `query`, ignoring case; all of them for none. */
export function matchBlocks(
  rows: readonly BlockRow[],
  query: string,
): Set<NodeId> {
  const q = query.trim().toLowerCase();
  return new Set(
    rows
      .filter(
        (r) =>
          q === "" ||
          r.name.toLowerCase().includes(q) ||
          r.kindLabel.includes(q),
      )
      .map((r) => r.id),
  );
}
