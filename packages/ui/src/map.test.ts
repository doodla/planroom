import type { MapNode, MapView } from "@planroom/contract";
import { expect, test } from "vitest";

import {
  blocksOf,
  edgesOf,
  kindLabel,
  matchBlocks,
  structureHash,
  viewKey,
} from "./map";

function node(id: string, over: Partial<MapNode> = {}): MapNode {
  return {
    id,
    kind: "pack.kind_name",
    tone: null,
    name: id.split(":").at(-1) ?? id,
    level: "container",
    parent: null,
    counts: {},
    position: null,
    ghost: false,
    claimed_by: null,
    ...over,
  };
}

// The code pack's ladder, as a `--detail graph` reply carries it.
const C4 = ["intent", "system", "container", "component", "code"].map((id) => ({
  id,
  label: id,
  one: `{count} ${id}`,
  many: `{count} ${id}s`,
  apart: id === "intent",
  pack: id === "intent" ? "kernel" : "code",
}));

function view(over: Partial<MapView> = {}): MapView {
  return {
    level: "container",
    levels: C4,
    lens: "all",
    focus: null,
    nodes: [
      node("a:svc:shop", {
        tone: "service",
        counts: { code: 21, component: 7 },
      }),
      node("b:db:orders-db", { tone: "store", name: "orders\u202Edb" }),
    ],
    edges: [
      {
        kind: "pack.queue_publish",
        from: "a:svc:shop",
        to: "b:db:orders-db",
        count: 3,
        evidence_sample: [],
        children: [],
      },
      {
        kind: "pack.http",
        from: "a:svc:shop",
        to: "x:gone:x",
        count: 1,
        evidence_sample: [],
        children: [],
      },
    ],
    hidden: 0,
    empty: null,
    ...over,
  };
}

test("a kind reads as words without its pack", () => {
  expect(kindLabel("pack.queue_publish")).toBe("queue publish");
  expect(kindLabel("bare")).toBe("bare");
});

test("blocks carry their tone, sanitized names and counts from the top level down", () => {
  const [shop, db] = blocksOf(view());
  expect(shop).toMatchObject({
    tone: "service",
    holds: true,
    counts: [
      { level: "component", count: 7 },
      { level: "code", count: 21 },
    ],
    kindLabel: "kind name",
  });
  expect(db).toMatchObject({
    tone: "store",
    holds: false,
    name: "orders⟨U+202E⟩db",
  });
});

test("a block with no tone keeps none, so it draws as a card", () => {
  expect(blocksOf(view({ nodes: [node("c:x:c")] }))[0]?.tone).toBeNull();
});

test("edges keep their counts and drop an end that isn't on the map", () => {
  expect(edgesOf(view())).toEqual([
    {
      id: "a:svc:shop>b:db:orders-db>pack.queue_publish",
      from: "a:svc:shop",
      to: "b:db:orders-db",
      kindLabel: "queue publish",
      count: 3,
    },
  ]);
});

test("the structure hash ignores names, counts, positions and order, and changes with the graph", () => {
  const base = structureHash(view());
  const renamed = view();
  renamed.nodes.reverse();
  renamed.nodes[0] = {
    ...renamed.nodes[0]!,
    name: "renamed",
    position: { x: 4, y: 2 },
  };
  expect(structureHash(renamed)).toBe(base);
  expect(structureHash(view({ edges: [] }))).not.toBe(base);
  expect(structureHash(view({ nodes: [node("a:svc:shop")] }))).not.toBe(base);
  expect(base).toMatch(/^[0-9a-f]{16}$/);
});

test("the view key names the level, lens and focus", () => {
  expect(viewKey(view())).toBe("container|all|");
  expect(viewKey(view({ focus: "a:svc:shop" }))).toBe(
    "container|all|a:svc:shop",
  );
});

test("search matches a block's name or kind, ignoring case", () => {
  const rows = blocksOf(view());
  expect(matchBlocks(rows, "SHOP")).toEqual(new Set(["a:svc:shop"]));
  expect(matchBlocks(rows, "kind")).toEqual(
    new Set(["a:svc:shop", "b:db:orders-db"]),
  );
  expect(matchBlocks(rows, "  ").size).toBe(2);
  expect(matchBlocks(rows, "nothing").size).toBe(0);
});

test("block_counts_follow_the_ladder", () => {
  const rung = (id: string, apart = false) => ({
    id,
    label: id,
    one: `{count} ${id}`,
    many: `{count} ${id}s`,
    apart,
    pack: "toy",
  });
  const [block] = blocksOf(
    view({
      level: "planet",
      levels: [
        rung("intent", true),
        rung("galaxy"),
        rung("planet"),
        rung("moon"),
        rung("dust"),
      ],
      nodes: [
        node("a:x:a", {
          level: "planet",
          counts: { dust: 9, zzz: 1, moon: 2, aaa: 4, galaxy: 0 },
        }),
      ],
    }),
  );
  expect(block?.counts).toEqual([
    { level: "moon", count: 2 },
    { level: "dust", count: 9 },
    { level: "aaa", count: 4 },
    { level: "zzz", count: 1 },
  ]);
  expect(block?.level).toBe("planet");
});
