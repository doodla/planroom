import type { MapView } from "@planroom/contract";
import { describe, expect, test } from "vitest";

import {
  editRequest,
  forbidChoices,
  moveTargets,
  pendingLine,
  pendingMarks,
  relationKinds,
  undoRequest,
} from "./ops";
import type { PendingEdit } from "./pending";

const node = (id: string, name: string, parent: string | null = null) => ({
  id,
  kind: "k.block",
  name,
  level: "l.one",
  parent,
  counts: {},
  ghost: false,
});

const edge = (kind: string, from: string, to: string) => ({
  kind,
  from,
  to,
  count: 1,
  evidence_sample: [],
  children: [],
});

function map(kinds: [string, string, string][] = []): MapView {
  return {
    level: "l.one",
    lens: "all",
    nodes: [node("a", "Alpha"), node("b", "Beta"), node("c", "Gamma", "b")],
    edges: kinds.map(([k, f, t]) => edge(k, f, t)),
    hidden: 0,
    layout: { view_key: "map:x:all", structure_hash: "h" },
  } as unknown as MapView;
}

describe("edit leaves", () => {
  test("each_gesture_builds_its_edit_leaf", () => {
    const m = map();
    expect(editRequest({ op: "rename", id: "a", name: "A2" }, m, 7)).toEqual({
      op: "rename",
      id: "a",
      name: "A2",
      base: 7,
    });
    expect(
      editRequest({ op: "group", name: "G", ids: ["b", "a"] }, m, 7),
    ).toEqual({ op: "group", name: "G", ids: ["b", "a"], base: 7 });
    expect(editRequest({ op: "hide", id: "a" }, m, 7)).toEqual({
      op: "hide",
      id: "a",
      base: 7,
    });
    expect(
      editRequest({ op: "pin", id: "a", x: 10.126, y: 20.004 }, m, 7),
    ).toEqual({
      op: "pin",
      id: "a",
      view: "map:x:all",
      x: 10.13,
      y: 20,
      base: 7,
    });
    expect(editRequest({ op: "move", id: "a", parent: "b" }, m, 7)).toEqual({
      op: "move",
      id: "a",
      parent: "b",
      base: 7,
    });
    expect(
      editRequest({ op: "relate", from: "a", to: "b", kind: "k.one" }, m, 7),
    ).toEqual({ op: "relate", from: "a", to: "b", kind: "k.one", base: 7 });
    expect(
      editRequest({ op: "forbid", from: "a", to: "b", kind: "k.one" }, m, 7),
    ).toEqual({ op: "forbid", from: "a", to: "b", kind: "k.one", base: 7 });
    expect(
      undoRequest({
        command: "edit",
        args: ["--base", "9", "undo", "E1"],
        label: "",
      }),
    ).toEqual({ op: "undo", event: "E1", base: 9 });
    expect(() =>
      undoRequest({ command: "edit", args: ["hide", "a"], label: "" }),
    ).toThrow();
    expect(pendingLine({ op: "hide", id: "a" }, m)).toBe("Your edit to Alpha");
    expect(pendingLine({ op: "group", name: "G", ids: ["x:y:gone"] }, m)).toBe(
      "Your edit to gone",
    );
  });

  test("relate_offers_only_the_kinds_the_view_draws", () => {
    const m = map([
      ["k.two", "a", "b"],
      ["k.one", "b", "a"],
      ["k.one", "a", "c"],
    ]);
    expect(relationKinds(m)).toEqual(["k.one", "k.two"]);
    expect(relationKinds(map())).toEqual([]);
    expect(moveTargets(m, "b").map((n) => n.id)).toEqual(["a"]);
    expect(forbidChoices(m, "a", "b")).toEqual([
      { from: "b", to: "a", kind: "k.one" },
      { from: "a", to: "b", kind: "k.two" },
    ]);
  });

  test("pending_edits_mark_the_blocks_they_touch", () => {
    const at = (
      idem: string,
      request: PendingEdit["request"],
    ): PendingEdit => ({
      idem,
      client: "tab:0123456789abcdef",
      request,
      line: "",
      at: 1,
    });
    const marks = pendingMarks([
      at("1", { op: "rename", id: "a", name: "A2" }),
      at("2", { op: "hide", id: "b" }),
      at("3", { op: "pin", id: "c", view: "v", x: 1, y: 2 }),
      at("4", { op: "relate", from: "a", to: "c", kind: "k.one" }),
      at("5", { op: "group", name: "G", ids: ["x", "y"] }),
      at("6", { op: "undo", event: "E1" }),
      at("7", { op: "pin", id: "d", view: "v", x: 3, y: 4 }),
    ]);
    // Every block an unsent edit touches carries the mark, a hidden or pinned one too.
    expect([...marks.marked].sort()).toEqual(["a", "b", "c", "d", "x", "y"]);
    expect(marks.names.get("a")).toBe("A2");
    expect([...marks.hidden]).toEqual(["b"]);
    expect(marks.pinned.get("c")).toEqual({ x: 1, y: 2 });
    expect(marks.pinned.get("d")).toEqual({ x: 3, y: 4 });
    expect(marks.chalk).toEqual([{ idem: "4", from: "a", to: "c" }]);
  });
});
