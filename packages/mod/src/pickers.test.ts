import type { Picker } from "@planroom/contract";
import { expect, test } from "vitest";

import lensReply from "../../../tests/golden/cli/focus-picker-lens.json";
import stageReply from "../../../tests/golden/cli/focus-picker-stage.json";
import { closes, pick, pickerArgv } from "./pickers";

const lens = (lensReply as unknown as { result: Picker }).result;
const stage = (stageReply as unknown as { result: Picker }).result;

test("a_pick_is_the_handle_its_key_names", () => {
  // The card's four take 1 to 4; the fifth entry takes the pool's first key.
  expect(pick(lens, "1")).toEqual(lens.entries[0]?.handle);
  expect(pick(lens, "2")).toEqual(lens.entries[1]?.handle);
  expect(pick(lens, "5")).toEqual(lens.entries[4]?.handle);
  // A key no entry holds names nothing.
  expect(pick(lens, "6")).toBeNull();
  expect(pick(lens, "x")).toBeNull();
  // A stage is a label with no key; lint keeps the key of its place, the seventh entry.
  expect(pick(stage, "1")).toBeNull();
  expect(pick(stage, "7")).toEqual(stage.entries[6]?.handle);
  expect(pick(stage, "7")?.command).toBe("intent lint");
});

test("any_other_action_key_closes_the_picker", () => {
  for (const key of ["x", "z", "u", "e", "p", "i"]) {
    expect(closes(key, "l"), key).toBe(true);
  }
  // Its own key opens it again; it never closes it.
  expect(closes("l", "l")).toBe(false);
  expect(closes("i", "i")).toBe(false);
});

test("a_picker_is_asked_for_by_name_and_the_project_only_when_one_was_picked", () => {
  expect(pickerArgv("lens", null)).toEqual(["focus", "--picker", "lens"]);
  expect(pickerArgv("stage", "plan:a:project")).toEqual([
    "focus",
    "--picker",
    "stage",
    "--project",
    "plan:a:project",
  ]);
});
