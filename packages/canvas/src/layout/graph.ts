/**
 * The map as an ELK graph and back (architecture.md, "The canvas", Layout; ts-stack §3): layered,
 * top down, `SEPARATE_CHILDREN` so a compound lays out on its own, with fixed block sizes so no
 * layout waits on the DOM. Positions are data: computed once in the worker, stored through the
 * transport, then served with the graph.
 */

import type { NodeId, Position } from "@planroom/contract";
import type { BlockRow, EdgeRow } from "@planroom/ui";

/** Every block's box, in CSS pixels at zoom 1. */
export const BLOCK = { width: 200, height: 92 } as const;

export interface ElkNodeIn {
  id: string;
  width: number;
  height: number;
}

export interface ElkEdgeIn {
  id: string;
  sources: string[];
  targets: string[];
}

export interface ElkGraphIn {
  id: string;
  layoutOptions: Record<string, string>;
  children: ElkNodeIn[];
  edges: ElkEdgeIn[];
}

export interface ElkGraphOut {
  children?: { id: string; x?: number; y?: number }[];
}

export const LAYOUT_OPTIONS: Readonly<Record<string, string>> = {
  "elk.algorithm": "layered",
  "elk.direction": "DOWN",
  "elk.hierarchyHandling": "SEPARATE_CHILDREN",
  "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
  "elk.spacing.nodeNode": "40",
  "elk.layered.spacing.nodeNodeBetweenLayers": "80",
  "elk.separateConnectedComponents": "true",
  "elk.aspectRatio": "1.3",
};

/** The ELK input for the blocks and lifted edges; one edge per connected pair. */
export function elkGraph(
  blocks: readonly BlockRow[],
  edges: readonly EdgeRow[],
): ElkGraphIn {
  const pairs = new Set<string>();
  const elkEdges: ElkEdgeIn[] = [];
  for (const e of edges) {
    const key = `${e.from}\n${e.to}`;
    if (e.from === e.to || pairs.has(key)) {
      continue;
    }
    pairs.add(key);
    elkEdges.push({
      id: `e${elkEdges.length}`,
      sources: [e.from],
      targets: [e.to],
    });
  }
  return {
    id: "root",
    layoutOptions: { ...LAYOUT_OPTIONS },
    children: blocks.map((b) => ({
      id: b.id,
      width: BLOCK.width,
      height: BLOCK.height,
    })),
    edges: elkEdges,
  };
}

/** ELK's output as one position per block, rounded to whole pixels. */
export function positionsOf(out: ElkGraphOut): Record<NodeId, Position> {
  const positions: Record<NodeId, Position> = {};
  for (const child of out.children ?? []) {
    positions[child.id] = {
      x: Math.round(child.x ?? 0),
      y: Math.round(child.y ?? 0),
    };
  }
  return positions;
}

/** Whether every block already has a stored position, so no layout runs. */
export function hasLayout(blocks: readonly BlockRow[]): boolean {
  return blocks.every((b) => b.position !== null);
}

/**
 * Each block's position: its stored one when it has one, which a person may have pinned, and the
 * computed one otherwise. ELK doesn't know pins, so a pin on a structure with no cached layout can
 * overlap a computed block.
 */
export function placed(
  blocks: readonly BlockRow[],
  computed: Readonly<Record<NodeId, Position>>,
): Record<NodeId, Position> {
  const out: Record<NodeId, Position> = {};
  for (const b of blocks) {
    const p = b.position ?? computed[b.id];
    if (p) {
      out[b.id] = p;
    }
  }
  return out;
}
