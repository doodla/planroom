/**
 * The environment every engine run the mod starts names: what it blanks, so a value the session
 * inherited never changes what a run does.
 */

import { expect, test, vi } from "vitest";

import { engineEnv } from "./live";

// live.ts reads and writes the mod's atoms through the host's module, which only a session
// provides; the environment it builds touches neither. vitest hoists this above the imports.
vi.mock("claude-code", () => ({
  atom: (key: unknown) => key,
  read: vi.fn(),
  update: vi.fn(),
}));

test("every_engine_run_blanks_raw_output_and_the_engine_pick", async () => {
  const env = await engineEnv();
  // An empty value reads as unset, so a PLANROOM_RAW=1 in the session's own environment never
  // turns redaction off for a tool result, /room or the pane.
  expect(env.PLANROOM_RAW).toBe("");
  expect(env.PLANROOM_ENGINE).toBe("");
  expect(env.PLANROOM_RELEASE_BASE_URL).toBe("");
  // A debug engine's opener seam reads empty as none, so a press of open never runs a program
  // the session's environment named.
  expect(env.PLANROOM_TEST_OPENER).toBe("");
});
