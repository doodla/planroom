import type { AskItem } from "@planroom/contract";
import { expect, test } from "vitest";

import { isLensItem, lensEntries, lensLabel } from "./lens-switcher";

const item = (
  id: string,
  name: string,
  handles: AskItem["handles"],
): AskItem => ({ id, name, why: "", handles });

const lens = (id: string, name = id): AskItem =>
  item(id, name, [
    { command: "map", args: ["--lens", id], label: `Show ${name}` },
  ]);

// The `ask` listing alone: kinds, lenses and questions, each with the handle that drills in.
const LISTING: AskItem[] = [
  item("pack.kind", "a kind", [
    { command: "ask", args: ["--kind", "pack.kind"], label: "" },
  ]),
  lens("all", "Everything"),
  lens("calls", "Calls"),
  item("why", "Why", [{ command: "ask", args: ["why"], label: "" }]),
  // Two handles, or a map handle with no lens, is not a lens entry.
  item("pair", "Pair", [
    { command: "map", args: ["--lens", "pair"], label: "" },
    { command: "map", args: ["--lens", "pair"], label: "" },
  ]),
  item("plain", "Plain", [{ command: "map", args: [], label: "" }]),
];

test("the_lens_entries_are_the_listings_map_lens_items", () => {
  expect(LISTING.filter(isLensItem).map((i) => i.id)).toEqual(["all", "calls"]);
  expect(lensEntries(LISTING, "calls")).toEqual([
    { key: "all", label: "Everything", current: false },
    { key: "calls", label: "Calls", current: true },
  ]);
  // Names are engine text: control and bidi characters show as markers.
  const loud = lensEntries([lens("x", "a‮b")], null);
  expect(loud[0]?.label).not.toContain("‮");
  expect(lensLabel(LISTING, "calls")).toBe("Calls");
});

test("before_the_listing_answers_the_entries_are_the_current_lens_alone", () => {
  expect(lensEntries([], "calls")).toEqual([
    { key: "calls", label: "calls", current: true },
  ]);
  expect(lensEntries([], null)).toEqual([]);
  expect(lensLabel([], "calls")).toBe("calls");
  expect(lensLabel([], null)).toBe("");
  // A listing that does not hold the current lens still labels it by its ID.
  expect(lensLabel(LISTING, "mine")).toBe("mine");
});
