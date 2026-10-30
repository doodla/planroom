import type { LevelVocab } from "@planroom/contract";
import { expect, test } from "vitest";

import {
  alreadyShown,
  atTop,
  crumbsAt,
  diveOf,
  mapRequest,
  topOf,
  viewOf,
} from "./levels";

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
  expect(viewOf(C4, OPEN)?.level).toBe("container");
  // A handle that names no level says nothing about it: the view is the top.
  expect(viewOf(C4, { command: "map", args: [], label: "Back" })).toEqual({});
  expect(
    viewOf(C4, { command: "scan", args: ["."], label: "Map it" }),
  ).toBeUndefined();
  // Once its level shows, the next action has nothing left to do.
  const shown = (level: string | null) => ({ level, lens: "all", focus: null });
  expect(alreadyShown(C4, OPEN, shown("system"))).toBe(false);
  expect(alreadyShown(C4, OPEN, shown("container"))).toBe(true);
  expect(
    alreadyShown(C4, { command: "scan", args: [], label: "" }, shown(null)),
  ).toBe(false);
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
  expect(viewOf(ladder, open("belt"))?.level).toBe("belt");
  expect(viewOf(ladder, open("plan"))?.level).toBe("plan");
  expect(viewOf(ladder, open("container"))?.level).toBeNull();
});

test("a_handles_view_keeps_its_level_lens_and_focus", () => {
  const map = (...args: string[]) => ({ command: "map", args, label: "" });
  expect(
    viewOf(
      C4,
      map("--level", "component", "--lens", "calls", "--focus", "a:b:c"),
    ),
  ).toEqual({ level: "component", lens: "calls", focus: "a:b:c" });
  // Each part is present only when the handle names it.
  expect(viewOf(C4, map("--lens", "calls"))).toEqual({ lens: "calls" });
  expect(viewOf(C4, map("--focus", "a:b:c"))).toEqual({ focus: "a:b:c" });
  expect(viewOf(C4, map())).toEqual({});
  // alreadyShown compares the level and the focus, and the lens only when it is named.
  const at = { level: "component", lens: "calls", focus: "a:b:c" };
  expect(
    alreadyShown(C4, map("--level", "component", "--focus", "a:b:c"), at),
  ).toBe(true);
  expect(
    alreadyShown(C4, map("--level", "component", "--focus", "x:y:z"), at),
  ).toBe(false);
  expect(
    alreadyShown(
      C4,
      map("--level", "component", "--focus", "a:b:c", "--lens", "other"),
      at,
    ),
  ).toBe(false);
});

test("the_top_is_the_first_level_not_apart", () => {
  const ladder = [rung("plan", "the plan", true), rung("galaxy"), rung("star")];
  expect(topOf(ladder)?.id).toBe("galaxy");
  expect(topOf([rung("plan", "p", true)])).toBeNull();
  expect(topOf([])).toBeNull();
  const top = topOf(ladder)?.id ?? "";
  expect(atTop(null, null, top)).toBe(true);
  expect(atTop("galaxy", null, top)).toBe(true);
  expect(atTop("star", null, top)).toBe(false);
  expect(atTop("galaxy", "a:b:c", top)).toBe(false);
  // Nothing at the top is asked of the engine; a lens picked at the top names the level.
  expect(mapRequest({ level: null, lens: null, trail: [] }, top)).toEqual({
    detail: "graph",
  });
  expect(mapRequest({ level: null, lens: "calls", trail: [] }, top)).toEqual({
    detail: "graph",
    level: "galaxy",
    lens: "calls",
  });
  const trail = [{ id: "a:b:c", name: "c", level: null }];
  expect(mapRequest({ level: "star", lens: null, trail }, top)).toEqual({
    detail: "graph",
    level: "star",
    focus: "a:b:c",
  });
});

test("a_dive_opens_the_entry_after_the_shown_one_and_an_apart_level_itself", () => {
  const ladder = [
    rung("plan", "the plan", true),
    rung("galaxy"),
    rung("star"),
    rung("rock"),
  ];
  const node = (counts: Record<string, number>) => ({ id: "n", counts });
  expect(diveOf(ladder, "galaxy", node({ star: 2 }))).toBe("star");
  expect(diveOf(ladder, "star", node({ rock: 1 }))).toBe("rock");
  // Nothing below the last entry, no count, or a count of none: no dive.
  expect(diveOf(ladder, "rock", node({ rock: 3 }))).toBeNull();
  expect(diveOf(ladder, "galaxy", node({}))).toBeNull();
  expect(diveOf(ladder, "galaxy", node({ star: 0 }))).toBeNull();
  // At the apart level the block opens that level itself.
  expect(diveOf(ladder, "plan", node({ plan: 4 }))).toBe("plan");
  expect(diveOf(ladder, "plan", node({}))).toBeNull();
});
