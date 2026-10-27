import type { Empty } from "@planroom/contract";
import { expect, test } from "vitest";

import { emptyWords } from "./engine-empty";

const CHECK_AGAIN = { command: "focus", args: [], label: "Check again" };

test("an_empty_state_draws_its_line_beside_its_short_action", () => {
  const unborn: Empty = {
    reason: "repos_unborn",
    line: "No commits yet in orders, worker, so there's nothing to map. Make a first commit, then check again.",
    next: CHECK_AGAIN,
  };
  expect(emptyWords(unborn)).toEqual({
    line: unborn.line,
    label: "Check again",
  });
  // An older engine that sends no line leaves the label alone.
  expect(
    emptyWords({ reason: "no_repos", line: null, next: CHECK_AGAIN }),
  ).toEqual({ line: undefined, label: "Check again" });
  expect(
    emptyWords({ reason: "no_repos", next: CHECK_AGAIN }).line,
  ).toBeUndefined();
});

test("an_empty_states_line_and_label_pass_the_display_sanitizing_pass", () => {
  const words = emptyWords({
    reason: "repos_unreadable",
    line: "git can't read evil‮gpj.exe\u001b[2J here",
    next: { ...CHECK_AGAIN, label: "Check​ again" },
  });
  expect(words.line).toBe("git can't read evil⟨U+202E⟩gpj.exe\\x1b[2J here");
  expect(words.label).toBe("Check⟨U+200B⟩ again");
});
