import { describe, expect, test } from "vitest";

import { requestOf } from "./request";

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
