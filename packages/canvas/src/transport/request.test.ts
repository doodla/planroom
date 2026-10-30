import { describe, expect, test } from "vitest";

import type { PartialViews } from "./types";
import { canRun, requestOf } from "./request";

describe("requestOf", () => {
  test("the empty map's scan names its directory as the dir field", () => {
    expect(
      requestOf({ command: "scan", args: ["/home/you/shop"], label: "Map" }),
    ).toEqual({ dir: "/home/you/shop" });
  });

  test("flags take their values, bare flags are true, dashes become underscores", () => {
    expect(
      requestOf({
        command: "map",
        args: ["--level", "container", "--detail=graph", "--dry-run"],
        label: "Open",
      }),
    ).toEqual({ level: "container", detail: "graph", dry_run: true });
  });

  test("a positional argument a command has no field for is refused", () => {
    expect(() =>
      requestOf({ command: "map", args: ["orders"], label: "Open" }),
    ).toThrow(/no positional argument/);
  });
});

describe("canRun", () => {
  const partial = (
    serves: (c: string, r: object) => boolean,
  ): PartialViews => ({
    serves,
    requested: () => ({ state: "unavailable", line: "", problem: null }),
    canAsk: () => false,
    ask: () => Promise.resolve(),
    note: () => null,
  });
  const map = (...args: string[]) => ({ command: "map", args, label: "" });

  test("a transport that holds every view runs every handle", () => {
    expect(canRun(null, map("--focus", "a:b:c"))).toBe(true);
    expect(canRun(null, { command: "scan", args: [], label: "" })).toBe(true);
  });

  test("a_partial_transport_runs_no_focused_map", () => {
    const none = partial(() => false);
    expect(canRun(none, map("--focus", "a:b:c"))).toBe(false);
    // Any other map handle becomes a request state, so it can run.
    expect(canRun(none, map("--level", "container"))).toBe(true);
    expect(canRun(none, map())).toBe(true);
    // Any other command runs when the transport serves its request.
    expect(canRun(none, { command: "scan", args: [], label: "" })).toBe(false);
    const asks: [string, object][] = [];
    const some = partial((c, r) => (asks.push([c, r]), c === "ask"));
    expect(
      canRun(some, { command: "ask", args: ["--question", "why"], label: "" }),
    ).toBe(true);
    expect(asks).toEqual([["ask", { question: "why" }]]);
  });
});
