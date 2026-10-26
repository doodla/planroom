import type { BlockRow, EdgeRow } from "@planroom/ui";
import ELK from "elkjs/lib/elk.bundled.js";
import { expect, test } from "vitest";

import {
  BLOCK,
  elkGraph,
  hasLayout,
  placed,
  positionsOf,
  type ElkGraphOut,
} from "./graph";

function block(id: string, position: BlockRow["position"] = null): BlockRow {
  return {
    id,
    name: id,
    kindLabel: "k",
    tone: null,
    level: "container",
    counts: [],
    holds: false,
    position,
    ghost: false,
  };
}

function edge(from: string, to: string): EdgeRow {
  return { id: `${from}>${to}`, from, to, kindLabel: "k", count: 1 };
}

const blocks = ["a", "b", "c", "d", "e"].map((id) => block(id));
const edges = [
  edge("a", "b"),
  edge("a", "b"),
  edge("b", "c"),
  edge("a", "c"),
  edge("d", "d"),
];

test("the graph has one sized node per block and one edge per connected pair", () => {
  const g = elkGraph(blocks, edges);
  expect(g.children).toHaveLength(5);
  expect(g.children[0]).toEqual({
    id: "a",
    width: BLOCK.width,
    height: BLOCK.height,
  });
  expect(g.edges.map((e) => [e.sources[0], e.targets[0]])).toEqual([
    ["a", "b"],
    ["b", "c"],
    ["a", "c"],
  ]);
  expect(g.layoutOptions["elk.hierarchyHandling"]).toBe("SEPARATE_CHILDREN");
});

test("ELK places every block without overlap, top down along the edges", async () => {
  const out = (await new ELK().layout(elkGraph(blocks, edges))) as ElkGraphOut;
  const at = positionsOf(out);
  expect(Object.keys(at).sort()).toEqual(["a", "b", "c", "d", "e"]);
  expect(at.a!.y).toBeLessThan(at.b!.y);
  expect(at.b!.y).toBeLessThan(at.c!.y);
  const ids = Object.keys(at);
  for (const [i, p] of ids.entries()) {
    for (const q of ids.slice(i + 1)) {
      const [m, n] = [at[p]!, at[q]!];
      const apart =
        Math.abs(m.x - n.x) >= BLOCK.width ||
        Math.abs(m.y - n.y) >= BLOCK.height;
      expect(apart, `${p} and ${q} overlap`).toBe(true);
    }
  }
});

test("stored positions win over computed ones, and a full set needs no layout", () => {
  const stored = [block("a", { x: 5, y: 6 }), block("b")];
  expect(hasLayout(stored)).toBe(false);
  expect(hasLayout([block("a", { x: 0, y: 0 })])).toBe(true);
  expect(placed(stored, { a: { x: 100, y: 100 }, b: { x: 7, y: 8 } })).toEqual({
    a: { x: 5, y: 6 },
    b: { x: 7, y: 8 },
  });
});
