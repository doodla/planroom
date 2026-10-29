import type { LevelVocab } from "@planroom/contract";
import { expect, test } from "vitest";

import { alreadyShown, crumbsAt, handleLevel } from "./levels";

function rung(id: string, label = id, apart = false): LevelVocab {
  return {
    id,
    label,
    one: `{count} ${label}`,
    many: `{count} ${label}s`,
    apart,
    pack: "p",
  };
}

// The code pack's ladder, as a `--detail graph` reply carries it.
const C4 = [
  rung("intent", "intent", true),
  rung("system"),
  rung("container"),
  rung("component"),
  rung("code"),
];

const OPEN = {
  command: "map",
  args: ["--level", "container"],
  label: "Open the containers",
};

test("ux-4: the breadcrumb comes from the map's level, the workspace on top", () => {
  expect(crumbsAt(C4, null, "shop")).toEqual([{ level: null, label: "shop" }]);
  expect(crumbsAt(C4, "container", "shop")).toEqual([
    { level: null, label: "shop" },
    { level: "container", label: "container" },
  ]);
  expect(crumbsAt(C4, "code", "shop").map((c) => c.label)).toEqual([
    "shop",
    "container",
    "component",
    "code",
  ]);
  // The plan sits beside the system: its one lid above is the top.
  expect(crumbsAt(C4, "intent", "shop")).toEqual([
    { level: null, label: "shop" },
    { level: "intent", label: "intent" },
  ]);
});

test("ux-4: a map handle names its level, the top when it names none", () => {
  expect(handleLevel(C4, OPEN)).toBe("container");
  expect(
    handleLevel(C4, { command: "map", args: [], label: "Back" }),
  ).toBeNull();
  expect(
    handleLevel(C4, { command: "scan", args: ["."], label: "Map it" }),
  ).toBeUndefined();
  // Once its level shows, the next action has nothing left to do.
  expect(alreadyShown(C4, OPEN, null)).toBe(false);
  expect(alreadyShown(C4, OPEN, "container")).toBe(true);
  expect(alreadyShown(C4, { command: "scan", args: [], label: "" }, null)).toBe(
    false,
  );
});

test("crumbs_come_from_the_ladder_with_the_apart_level_beside", () => {
  // An invented ladder: the apart level is no chain entry, the labels aren't the IDs, and
  // a pack's inserted level sits between two of the base pack's.
  const ladder = [
    rung("galaxy"),
    rung("plan", "the plan", true),
    rung("star", "solar system"),
    rung("belt", "asteroid belt"),
    rung("rock"),
  ];
  expect(crumbsAt(ladder, "rock", "home")).toEqual([
    { level: null, label: "home" },
    { level: "star", label: "solar system" },
    { level: "belt", label: "asteroid belt" },
    { level: "rock", label: "rock" },
  ]);
  // The apart level's one lid above is the top, wherever it sits in the list.
  expect(crumbsAt(ladder, "plan", "home")).toEqual([
    { level: null, label: "home" },
    { level: "plan", label: "the plan" },
  ]);
  // The ladder's top is the top-level map itself.
  expect(crumbsAt(ladder, "galaxy", "home")).toEqual([
    { level: null, label: "home" },
  ]);
  // A handle's level counts when the ladder holds it; any other is the top.
  const open = (level: string) => ({
    command: "map",
    args: ["--level", level],
    label: "",
  });
  expect(handleLevel(ladder, open("belt"))).toBe("belt");
  expect(handleLevel(ladder, open("plan"))).toBe("plan");
  expect(handleLevel(ladder, open("container"))).toBeNull();
});
