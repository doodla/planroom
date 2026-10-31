import type { Empty } from "@planroom/contract";
import { expect, test } from "vitest";

import { drawsHere, emptyWords, fillsThePage, inPane } from "./engine-empty";

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

test("a_handle_the_canvas_cannot_draw_names_the_pane", () => {
  expect(drawsHere({ command: "map", args: [], label: "x" })).toBe(true);
  expect(drawsHere({ command: "scan", args: [], label: "x" })).toBe(true);
  expect(drawsHere({ command: "focus", args: [], label: "x" })).toBe(true);
  expect(
    drawsHere({
      command: "schema",
      args: ["--command", "intent new"],
      label: "Start a plan here",
    }),
  ).toBe(false);
  const show = { command: "intent list", args: [], label: "Show the plan" };
  expect(drawsHere(show)).toBe(false);
  expect(inPane(show)).toBe(
    "Show the plan: in Claude Code, in Planroom's pane.",
  );
});

test("a_top_that_holds_a_plan_keeps_the_shell_and_its_project_shelf", () => {
  const show = { command: "intent list", args: [], label: "Show the plan" };
  expect(fillsThePage({ reason: "plan_only", line: "x", next: show })).toBe(
    false,
  );
  for (const reason of [
    "not_scanned",
    "no_repos",
    "repos_unborn",
    "repos_unreadable",
  ]) {
    expect(fillsThePage({ reason, next: CHECK_AGAIN }), reason).toBe(true);
  }
});
