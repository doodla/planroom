import { expect, test } from "vitest";

import { alreadyShown, crumbsAt, handleLevel } from "./levels";

const OPEN = {
  command: "map",
  args: ["--level", "container"],
  label: "Open the containers",
};

test("ux-4: the breadcrumb comes from the map's level, the workspace on top", () => {
  expect(crumbsAt(null, "shop")).toEqual([{ level: null, label: "shop" }]);
  expect(crumbsAt("container", "shop")).toEqual([
    { level: null, label: "shop" },
    { level: "container", label: "container" },
  ]);
  expect(crumbsAt("code", "shop").map((c) => c.label)).toEqual([
    "shop",
    "container",
    "component",
    "code",
  ]);
  // The plan sits beside the system: its one lid above is the top.
  expect(crumbsAt("intent", "shop")).toEqual([
    { level: null, label: "shop" },
    { level: "intent", label: "intent" },
  ]);
});

test("ux-4: a map handle names its level, the top when it names none", () => {
  expect(handleLevel(OPEN)).toBe("container");
  expect(handleLevel({ command: "map", args: [], label: "Back" })).toBeNull();
  expect(
    handleLevel({ command: "scan", args: ["."], label: "Map it" }),
  ).toBeUndefined();
  // Once its level shows, the next action has nothing left to do.
  expect(alreadyShown(OPEN, null)).toBe(false);
  expect(alreadyShown(OPEN, "container")).toBe(true);
  expect(alreadyShown({ command: "scan", args: [], label: "" }, null)).toBe(
    false,
  );
});
