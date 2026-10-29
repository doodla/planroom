import { MOD_STRINGS } from "@planroom/contract";
import { expect, test } from "vitest";

import { composeSections, promptSection, type PromptSection } from "./prompt";

/** Every engine status the mod's state can hold. */
const STATUSES = [
  "resolving",
  "fetching",
  "build_offered",
  "building",
  "failed",
  "unsupported_platform",
  "version_mismatch",
  "person_key_mismatch",
  "ready",
] as const;

test("section_is_byte_identical_across_states: one stable section, appended last, within its budget", () => {
  const hosts: PromptSection[][] = [
    [],
    [{ id: "intro", text: "You are Claude.", scope: "shared" }],
    [
      { id: "intro", text: "You are Claude.", scope: "shared" },
      { id: "tools", text: "Tools.", scope: "shared" },
      { id: "acme:policy", text: "Be kind.", scope: "session" },
    ],
    // A list that already holds a stale copy of the section, as after a hook above re-runs.
    [
      { id: "planroom:map", text: "an older text", scope: "session" },
      { id: "intro", text: "You are Claude.", scope: "shared" },
    ],
  ];
  const seen = new Set<string>();
  // The section takes no state, so every status, remote mode and turn composes the same bytes.
  // It takes no argument, so nothing of the state can reach it.
  expect(promptSection.length).toBe(0);
  for (const status of STATUSES) {
    for (const host of hosts) {
      const composed = composeSections(host);
      const ours = composed.filter((s) => s.id === "planroom:map");
      expect(ours, status).toHaveLength(1);
      expect(composed.at(-1)).toEqual(promptSection());
      seen.add(JSON.stringify(ours[0]));
    }
  }
  expect(seen.size).toBe(1);
  expect(promptSection()).toEqual({
    id: "planroom:map",
    text: MOD_STRINGS.prompt_section,
    scope: "session",
  });
  const bytes = new TextEncoder().encode(promptSection().text).length;
  expect(Math.ceil(bytes / 4)).toBeLessThanOrEqual(
    __PLANROOM_BUDGETS__.surface.section,
  );
  // It names the tools it describes and asks for base from the seq read.
  for (const said of ["room_query", "room_change", "meta.seq"]) {
    expect(promptSection().text).toContain(said);
  }
});
